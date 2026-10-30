//! Spend reports. Totals and breakdowns read the daily rollup; the request
//! log is only paged through, never aggregated.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::DbPool;
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::list::{List, ListQuery, Paged};
use crate::models::money::nanos_to_usd;
use crate::models::user::User;
use crate::services::access::team_access;

const DEFAULT_DAYS: i64 = 30;
const SERIES_GROUPS: [&str; 6] = ["model", "team", "key", "person", "end_user", "tag"];
const MAX_DAYS: i64 = 366;

#[derive(Debug, Clone, Deserialize)]
pub struct UsageQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    #[serde(flatten)]
    pub scope: Scope,
    /// What `series` splits by: `model` (the default), `team`, `key`,
    /// `person`, `end_user` or `tag`.
    pub group_by: Option<String>,
}

/// Whose spend a report counts. Each dimension keeps only the listed values
/// (every value when none is listed) and then leaves out the `exclude_`
/// ones: `team_id=4&exclude_key_id=17`, `key_id=3,8,12`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Scope {
    #[serde(default)]
    pub team_id: List<i64>,
    #[serde(default)]
    pub exclude_team_id: List<i64>,
    #[serde(default)]
    pub key_id: List<i64>,
    #[serde(default)]
    pub exclude_key_id: List<i64>,
    /// Public model names. Customers and tags are rolled up without the
    /// model, so with a model filter their breakdowns come back empty.
    #[serde(default)]
    pub model: List<String>,
    #[serde(default)]
    pub exclude_model: List<String>,
    /// The keys' spend while assigned to these people. Like the model,
    /// customers and tags do not carry it.
    #[serde(default)]
    pub person_id: List<i64>,
    #[serde(default)]
    pub exclude_person_id: List<i64>,
    /// The keys that carry one of these labels now, all their spend included.
    #[serde(default)]
    pub label_id: List<i64>,
    #[serde(default)]
    pub exclude_label_id: List<i64>,
}

impl Scope {
    /// Whether the model or the person is filtered, which the customer and
    /// tag rollups cannot answer.
    pub fn by_model_or_person(&self) -> bool {
        !(self.model.is_empty()
            && self.exclude_model.is_empty()
            && self.person_id.is_empty()
            && self.exclude_person_id.is_empty())
    }
}

/// Binds a [`Scope`] to `$5`…`$14` of [`filters`].
macro_rules! bind_scope {
    ($query:expr, $s:expr) => {
        $query
            .bind($s.team_id.json())
            .bind($s.key_id.json())
            .bind($s.model.json())
            .bind($s.person_id.json())
            .bind($s.label_id.json())
            .bind($s.exclude_team_id.json())
            .bind($s.exclude_key_id.json())
            .bind($s.exclude_model.json())
            .bind($s.exclude_person_id.json())
            .bind($s.exclude_label_id.json())
    };
}

/// SQL for "`column` is one of the JSON list `$n`", true when `$n` is null.
fn among(column: &str, n: u8, ints: bool) -> String {
    format!("(${n} IS NULL OR {column} IN ({}))", values(n, ints))
}

/// SQL for "`column` is not one of the JSON list `$n`". Rows without a
/// value (a personal key's team, a key of no one) are never left out.
fn not_among(column: &str, n: u8, ints: bool) -> String {
    format!(
        "(${n} IS NULL OR {column} IS NULL OR {column} NOT IN ({}))",
        values(n, ints)
    )
}

fn values(n: u8, ints: bool) -> String {
    let each = crate::db::each(&format!("${n}"));
    if ints {
        format!("SELECT CAST(value AS BIGINT) FROM {each}")
    } else {
        format!("SELECT value FROM {each}")
    }
}

/// Keys carrying (or not) one of the labels of list `$n`, now.
fn labelled(key: &str, n: u8, carrying: bool) -> String {
    let not = if carrying { "" } else { "NOT " };
    format!(
        "(${n} IS NULL OR {key} {not}IN (SELECT key_id FROM key_labels WHERE label_id IN ({})))",
        values(n, true)
    )
}

#[derive(Debug, Default, Serialize, Clone, Copy)]
pub struct Totals {
    pub cost_usd: f64,
    /// The same, exact: what reports add and convert.
    #[serde(skip)]
    pub cost_nanos: i64,
    pub requests: i64,
    pub failed_requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    /// Summed over the requests: divide by `requests` for the average.
    pub latency_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct Day {
    pub day: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct TeamUsage {
    /// `None` for personal keys, which bill to no team.
    pub team_id: Option<i64>,
    pub team_name: Option<String>,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct KeyUsage {
    pub key_id: i64,
    pub key_name: String,
    pub key_hint: String,
    pub team_name: Option<String>,
    #[serde(flatten)]
    pub totals: Totals,
}

/// Spend of one of a team's people; unassigned keys are left out.
#[derive(Debug, Serialize)]
pub struct PersonUsage {
    pub person_id: i64,
    pub person_name: String,
    pub team_name: Option<String>,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct ModelUsage {
    pub model_name: String,
    #[serde(flatten)]
    pub totals: Totals,
}

/// Spend of one end user (a customer).
#[derive(Debug, Serialize)]
pub struct EndUserUsage {
    pub end_user: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct TagUsage {
    pub tag: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct UsageReport {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub totals: Totals,
    /// The same filters over the equal window just before `from`.
    pub previous: Totals,
    /// Per day and per group of `group_by`; days without traffic are absent.
    pub series: Vec<SeriesPoint>,
    pub daily: Vec<Day>,
    pub by_team: Vec<TeamUsage>,
    pub by_key: Vec<KeyUsage>,
    pub by_person: Vec<PersonUsage>,
    pub by_model: Vec<ModelUsage>,
    pub by_end_user: Vec<EndUserUsage>,
    /// A request with several tags counts once under each.
    pub by_tag: Vec<TagUsage>,
}

/// One aggregated rollup row: the group label columns plus the sums.
#[derive(sqlx::FromRow)]
struct Row {
    label: String,
    id: Option<i64>,
    name: Option<String>,
    hint: Option<String>,
    team_name: Option<String>,
    cost_nanos: i64,
    requests: i64,
    failed_requests: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    cached_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    latency_ms: i64,
}

/// One day of one group.
#[derive(Debug, Serialize)]
pub struct SeriesPoint {
    pub day: String,
    /// Stable within a report: a model name, team or key id, customer, tag.
    /// Empty for personal keys when grouping by team, and for keys of no
    /// one when grouping by person.
    pub key: String,
    /// What to call it; `None` when its team or key was deleted.
    pub label: Option<String>,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(sqlx::FromRow)]
struct SeriesRow {
    day: String,
    key: String,
    label: Option<String>,
    #[sqlx(flatten)]
    sums: Sums,
}

/// The summed columns of `SUMS`.
#[derive(sqlx::FromRow)]
struct Sums {
    cost_nanos: i64,
    requests: i64,
    failed_requests: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    cached_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    latency_ms: i64,
}

impl Sums {
    fn totals(&self) -> Totals {
        Totals {
            cost_usd: nanos_to_usd(self.cost_nanos),
            cost_nanos: self.cost_nanos,
            requests: self.requests,
            failed_requests: self.failed_requests,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            cached_tokens: self.cached_tokens,
            cache_write_tokens: self.cache_write_tokens,
            reasoning_tokens: self.reasoning_tokens,
            latency_ms: self.latency_ms,
        }
    }
}

impl Row {
    fn totals(&self) -> Totals {
        Totals {
            cost_usd: nanos_to_usd(self.cost_nanos),
            cost_nanos: self.cost_nanos,
            requests: self.requests,
            failed_requests: self.failed_requests,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            cached_tokens: self.cached_tokens,
            cache_write_tokens: self.cache_write_tokens,
            reasoning_tokens: self.reasoning_tokens,
            latency_ms: self.latency_ms,
        }
    }
}

const SUMS: &str = "
    CAST(COALESCE(SUM(u.cost_nanos), 0) AS BIGINT) AS cost_nanos,
    CAST(COALESCE(SUM(u.requests), 0) AS BIGINT) AS requests,
    CAST(COALESCE(SUM(u.failed_requests), 0) AS BIGINT) AS failed_requests,
    CAST(COALESCE(SUM(u.prompt_tokens), 0) AS BIGINT) AS prompt_tokens,
    CAST(COALESCE(SUM(u.completion_tokens), 0) AS BIGINT) AS completion_tokens,
    CAST(COALESCE(SUM(u.cached_tokens), 0) AS BIGINT) AS cached_tokens,
    CAST(COALESCE(SUM(u.cache_write_tokens), 0) AS BIGINT) AS cache_write_tokens,
    CAST(COALESCE(SUM(u.reasoning_tokens), 0) AS BIGINT) AS reasoning_tokens,
    CAST(COALESCE(SUM(u.latency_ms), 0) AS BIGINT) AS latency_ms";

/// Rows `user` may see: everything for an admin; their teams' and their
/// personal keys' otherwise.
const VISIBLE: &str = "($3 OR u.user_id = $4
    OR u.team_id IN (SELECT team_id FROM team_members WHERE user_id = $4))";

/// The rollup rows of `table` a report covers. Binds: `$1`..`$2` the days,
/// `$3` admin, `$4` the user, then the [`Scope`] (`bind_scope`): `$5` teams,
/// `$6` keys, `$7` models, `$8` people, `$9` labels, and `$10`…`$14` the
/// same left out.
///
/// Customers and tags are rolled up without the model and the person: a
/// filter on either matches none of their rows rather than all of them.
fn filters(table: &str) -> String {
    let model_and_person = if table == "usage_daily" {
        [
            among("u.model_name", 7, false),
            among("u.person_id", 8, true),
            not_among("u.model_name", 12, false),
            not_among("u.person_id", 13, true),
        ]
        .join(" AND ")
    } else {
        "$7 IS NULL AND $8 IS NULL AND $12 IS NULL AND $13 IS NULL".to_string()
    };
    format!(
        "u.day >= $1 AND u.day <= $2 AND {VISIBLE}
         AND {} AND {} AND {} AND {} AND {model_and_person} AND {} AND {}",
        among("u.team_id", 5, true),
        among("u.key_id", 6, true),
        not_among("u.team_id", 10, true),
        not_among("u.key_id", 11, true),
        labelled("u.key_id", 9, true),
        labelled("u.key_id", 14, false),
    )
}

pub async fn report(pool: &DbPool, user: &User, query: &UsageQuery) -> AppResult<UsageReport> {
    let scope = &query.scope;
    for team_id in scope.team_id.0.iter().chain(&scope.exclude_team_id.0) {
        team_access(pool, user, *team_id).await?;
    }
    let to = query.to.unwrap_or_else(|| Utc::now().date_naive());
    let from = query
        .from
        .unwrap_or_else(|| to - chrono::Duration::days(DEFAULT_DAYS - 1));
    let days = (to - from).num_days() + 1;
    if days < 1 {
        return Err(AppError::Validation("`from` must not be after `to`".into()));
    }
    if days > MAX_DAYS {
        return Err(AppError::Validation(format!(
            "a report covers at most {MAX_DAYS} days"
        )));
    }

    let group_by = query.group_by.as_deref().unwrap_or("model");
    if !SERIES_GROUPS.contains(&group_by) {
        return Err(AppError::Validation(format!(
            "group_by must be one of {}",
            SERIES_GROUPS.join(", ")
        ))
        .with_field("group_by", FieldErrorCode::Invalid));
    }
    let group_in = |table: &str, select: &str, joins: &str, group_by: &str| {
        format!(
            "SELECT {select}, {SUMS}
             FROM {table} u {joins}
             WHERE {}
             GROUP BY {group_by}
             ORDER BY cost_nanos DESC, label",
            filters(table)
        )
    };
    let group = |select: &str, joins: &str, group_by: &str| {
        group_in("usage_daily", select, joins, group_by)
    };
    const PLAIN: &str = "CAST(0 AS BIGINT) AS id, CAST(NULL AS TEXT) AS name, \
                         CAST(NULL AS TEXT) AS hint, CAST(NULL AS TEXT) AS team_name";
    let fetch = |sql: String| {
        bind_scope!(
            sqlx::query_as::<_, Row>(sqlx::AssertSqlSafe(sql))
                .bind(from.to_string())
                .bind(to.to_string())
                .bind(user.is_admin())
                .bind(user.id),
            query.scope
        )
        .fetch_all(pool)
    };

    let previous_from = from - chrono::Duration::days(days);
    let previous_to = from - chrono::Duration::days(1);
    let previous = bind_scope!(
        sqlx::query_as::<_, Sums>(sqlx::AssertSqlSafe(format!(
            "SELECT {SUMS} FROM usage_daily u WHERE {}",
            filters("usage_daily")
        )))
        .bind(previous_from.to_string())
        .bind(previous_to.to_string())
        .bind(user.is_admin())
        .bind(user.id),
        query.scope
    )
    .fetch_one(pool)
    .await?
    .totals();

    // `group_by` is one of SERIES_GROUPS, so these are literals.
    let (table, columns, joins, grouping) = match group_by {
        // Who is billed: the team, or for a personal key its user.
        "team" => (
            "usage_daily",
            "CASE WHEN u.team_id IS NULL THEN 'user:' || CAST(u.user_id AS TEXT)
                  ELSE CAST(u.team_id AS TEXT) END AS key,
             COALESCE(t.name, us.name, us.email) AS label",
            "LEFT JOIN teams t ON t.id = u.team_id
             LEFT JOIN users us ON us.id = u.user_id AND u.team_id IS NULL",
            "CASE WHEN u.team_id IS NULL THEN 'user:' || CAST(u.user_id AS TEXT)
                  ELSE CAST(u.team_id AS TEXT) END, t.name, us.name, us.email",
        ),
        "key" => (
            "usage_daily",
            "CAST(u.key_id AS TEXT) AS key, k.name AS label",
            "LEFT JOIN api_keys k ON k.id = u.key_id",
            "u.key_id, k.name",
        ),
        "person" => (
            "usage_daily",
            "COALESCE(CAST(u.person_id AS TEXT), '') AS key, p.name AS label",
            "LEFT JOIN people p ON p.id = u.person_id",
            "u.person_id, p.name",
        ),
        "end_user" => (
            "usage_daily_end_users",
            "u.end_user AS key, u.end_user AS label",
            "",
            "u.end_user",
        ),
        "tag" => (
            "usage_daily_tags",
            "u.tag AS key, u.tag AS label",
            "",
            "u.tag",
        ),
        _ => (
            "usage_daily",
            "u.model_name AS key, u.model_name AS label",
            "",
            "u.model_name",
        ),
    };
    let series = bind_scope!(
        sqlx::query_as::<_, SeriesRow>(sqlx::AssertSqlSafe(format!(
            "SELECT u.day AS day, {columns}, {SUMS}
             FROM {table} u {joins}
             WHERE {}
             GROUP BY u.day, {grouping}
             ORDER BY u.day",
            filters(table)
        )))
        .bind(from.to_string())
        .bind(to.to_string())
        .bind(user.is_admin())
        .bind(user.id),
        query.scope
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| SeriesPoint {
        day: row.day,
        key: row.key,
        label: row.label,
        totals: row.sums.totals(),
    })
    .collect();

    let daily_rows = fetch(group(
        "u.day AS label, CAST(0 AS BIGINT) AS id, CAST(NULL AS TEXT) AS name, CAST(NULL AS TEXT) AS hint, CAST(NULL AS TEXT) AS team_name",
        "",
        "u.day",
    ))
    .await?;
    let team_rows = fetch(group(
        "COALESCE(t.name, '') AS label, u.team_id AS id, t.name AS name, CAST(NULL AS TEXT) AS hint, CAST(NULL AS TEXT) AS team_name",
        "LEFT JOIN teams t ON t.id = u.team_id",
        "u.team_id, t.name",
    ))
    .await?;
    let key_rows = fetch(group(
        "COALESCE(k.name, '') AS label, u.key_id AS id, k.name AS name, k.last4 AS hint, t.name AS team_name",
        "LEFT JOIN api_keys k ON k.id = u.key_id LEFT JOIN teams t ON t.id = u.team_id",
        "u.key_id, k.name, k.last4, t.name",
    ))
    .await?;
    let person_rows = fetch(group(
        "COALESCE(p.name, '') AS label, u.person_id AS id, p.name AS name, CAST(NULL AS TEXT) AS hint, t.name AS team_name",
        "LEFT JOIN people p ON p.id = u.person_id LEFT JOIN teams t ON t.id = u.team_id",
        "u.person_id, p.name, t.name",
    ))
    .await?;
    let model_rows = fetch(group(
        "u.model_name AS label, CAST(0 AS BIGINT) AS id, CAST(NULL AS TEXT) AS name, CAST(NULL AS TEXT) AS hint, CAST(NULL AS TEXT) AS team_name",
        "",
        "u.model_name",
    ))
    .await?;

    let end_user_rows = fetch(group_in(
        "usage_daily_end_users",
        &format!("u.end_user AS label, {PLAIN}"),
        "",
        "u.end_user",
    ))
    .await?;
    let tag_rows = fetch(group_in(
        "usage_daily_tags",
        &format!("u.tag AS label, {PLAIN}"),
        "",
        "u.tag",
    ))
    .await?;

    let mut totals = Totals::default();
    let mut by_day: HashMap<String, Totals> = HashMap::new();
    for row in &daily_rows {
        let day = row.totals();
        totals.cost_usd += day.cost_usd;
        totals.requests += day.requests;
        totals.failed_requests += day.failed_requests;
        totals.prompt_tokens += day.prompt_tokens;
        totals.completion_tokens += day.completion_tokens;
        totals.cached_tokens += day.cached_tokens;
        totals.cache_write_tokens += day.cache_write_tokens;
        totals.reasoning_tokens += day.reasoning_tokens;
        totals.latency_ms += day.latency_ms;
        by_day.insert(row.label.clone(), day);
    }
    // Recomputed from nanos so the sum of floats never drifts from the parts.
    totals.cost_nanos = daily_rows.iter().map(|r| r.cost_nanos).sum();
    totals.cost_usd = nanos_to_usd(totals.cost_nanos);

    let daily = from
        .iter_days()
        .take(days as usize)
        .map(|date| {
            let day = date.to_string();
            Day {
                totals: by_day.remove(&day).unwrap_or_default(),
                day,
            }
        })
        .collect();

    Ok(UsageReport {
        from,
        to,
        totals,
        previous,
        series,
        daily,
        by_team: team_rows
            .iter()
            .map(|row| TeamUsage {
                team_id: row.id,
                team_name: match (row.id, &row.name) {
                    (Some(id), None) => Some(format!("deleted team #{id}")),
                    (_, name) => name.clone(),
                },
                totals: row.totals(),
            })
            .collect(),
        by_key: key_rows
            .iter()
            .map(|row| {
                let id = row.id.unwrap_or_default();
                KeyUsage {
                    key_id: id,
                    key_name: row
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("deleted key #{id}")),
                    key_hint: format!("sk-...{}", row.hint.as_deref().unwrap_or("????")),
                    team_name: row.team_name.clone(),
                    totals: row.totals(),
                }
            })
            .collect(),
        by_person: person_rows
            .iter()
            .filter_map(|row| {
                let id = row.id?;
                Some(PersonUsage {
                    person_id: id,
                    person_name: row
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("deleted person #{id}")),
                    team_name: row.team_name.clone(),
                    totals: row.totals(),
                })
            })
            .collect(),
        by_model: model_rows
            .iter()
            .map(|row| ModelUsage {
                model_name: row.label.clone(),
                totals: row.totals(),
            })
            .collect(),
        by_end_user: end_user_rows
            .iter()
            .map(|row| EndUserUsage {
                end_user: row.label.clone(),
                totals: row.totals(),
            })
            .collect(),
        by_tag: tag_rows
            .iter()
            .map(|row| TagUsage {
                tag: row.label.clone(),
                totals: row.totals(),
            })
            .collect(),
    })
}

/// One rollup row as it is stored: a day of one key on one model.
#[derive(Debug, sqlx::FromRow)]
pub struct Line {
    pub day: String,
    pub key_id: i64,
    pub key_name: Option<String>,
    pub last4: Option<String>,
    pub team_name: Option<String>,
    /// For a personal key, whose it is.
    pub owner: Option<String>,
    pub person_name: Option<String>,
    pub model_name: String,
    pub requests: i64,
    pub failed_requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub cost_nanos: i64,
    pub latency_ms: i64,
}

/// The rollup rows behind a report over `from..=to`, oldest first. The
/// caller has already checked the range and the team.
pub async fn lines(
    pool: &DbPool,
    user: &User,
    query: &UsageQuery,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<Vec<Line>> {
    Ok(bind_scope!(
        sqlx::query_as::<_, Line>(sqlx::AssertSqlSafe(format!(
            "SELECT u.day, u.key_id, k.name AS key_name, k.last4, t.name AS team_name,
                COALESCE(us.name, us.email) AS owner, p.name AS person_name, u.model_name,
                u.requests, u.failed_requests, u.prompt_tokens, u.completion_tokens,
                u.cached_tokens, u.cache_write_tokens, u.reasoning_tokens, u.cost_nanos,
                u.latency_ms
         FROM usage_daily u
         LEFT JOIN api_keys k ON k.id = u.key_id
         LEFT JOIN teams t ON t.id = u.team_id
         LEFT JOIN users us ON us.id = u.user_id AND u.team_id IS NULL
         LEFT JOIN people p ON p.id = u.person_id
         WHERE {}
         ORDER BY u.day, t.name, k.name, u.key_id, u.model_name",
            filters("usage_daily")
        )))
        .bind(from.to_string())
        .bind(to.to_string())
        .bind(user.is_admin())
        .bind(user.id),
        query.scope
    )
    .fetch_all(pool)
    .await?)
}

/// Up to `limit` requests of `from..=to` (UTC days), oldest first, under the
/// report's filters. Requests do not carry the person, so a person filter
/// (kept or left out) matches none.
pub async fn requests(
    pool: &DbPool,
    user: &User,
    query: &UsageQuery,
    from: NaiveDate,
    to: NaiveDate,
    limit: i64,
) -> AppResult<Vec<LogEntry>> {
    let start = from.and_time(chrono::NaiveTime::MIN).and_utc();
    let end = (to + chrono::Duration::days(1))
        .and_time(chrono::NaiveTime::MIN)
        .and_utc();
    let sql = format!(
        "SELECT l.id, l.request_id, l.created_at, l.team_id, t.name AS team_name, l.user_id, l.key_id,
                l.model_id, l.end_user, l.tags AS tags_json,
                k.name AS key_name, k.last4, l.model_name, l.provider, l.status_code,
                l.prompt_tokens, l.completion_tokens, l.cached_tokens, l.cache_write_tokens,
                l.reasoning_tokens, l.endpoint, l.cost_nanos, l.latency_ms, l.stream, l.error
         FROM request_logs l
         LEFT JOIN teams t ON t.id = l.team_id
         LEFT JOIN api_keys k ON k.id = l.key_id
         WHERE l.created_at >= $1 AND l.created_at < $2
           AND ($3 OR l.user_id = $4
                OR l.team_id IN (SELECT team_id FROM team_members WHERE user_id = $4))
           AND {} AND {} AND {} AND {} AND {} AND {} AND {} AND {}
           AND $8 IS NULL AND $13 IS NULL
         ORDER BY l.created_at, l.id
         LIMIT $15",
        among("l.team_id", 5, true),
        among("l.key_id", 6, true),
        among("l.model_name", 7, false),
        labelled("l.key_id", 9, true),
        not_among("l.team_id", 10, true),
        not_among("l.key_id", 11, true),
        not_among("l.model_name", 12, false),
        labelled("l.key_id", 14, false),
    );
    Ok(bind_scope!(
        sqlx::query_as::<_, LogEntry>(sqlx::AssertSqlSafe(sql))
            .bind(start)
            .bind(end)
            .bind(user.is_admin())
            .bind(user.id),
        query.scope
    )
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Deserialize)]
/// Which requests the log shows. Like a report's [`Scope`], each list keeps
/// only its values (all when empty) and `exclude_` leaves values out; a
/// request with several tags matches a tag filter by any of them.
pub struct LogQuery {
    #[serde(default)]
    pub team_id: List<i64>,
    #[serde(default)]
    pub exclude_team_id: List<i64>,
    #[serde(default)]
    pub key_id: List<i64>,
    #[serde(default)]
    pub exclude_key_id: List<i64>,
    #[serde(default)]
    pub model: List<String>,
    #[serde(default)]
    pub exclude_model: List<String>,
    /// Keys that carry one of these labels now.
    #[serde(default)]
    pub label_id: List<i64>,
    #[serde(default)]
    pub exclude_label_id: List<i64>,
    #[serde(default)]
    pub end_user: List<String>,
    #[serde(default)]
    pub exclude_end_user: List<String>,
    #[serde(default)]
    pub tag: List<String>,
    #[serde(default)]
    pub exclude_tag: List<String>,
    /// `success` or `error`.
    pub status: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// Words in the stored request or reply, all of them, in any order and
    /// case. Only requests whose key keeps bodies can match.
    pub text: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub request_id: String,
    pub created_at: DateTime<Utc>,
    pub team_id: Option<i64>,
    pub team_name: Option<String>,
    pub user_id: Option<i64>,
    pub key_id: i64,
    /// The deployment that answered.
    pub model_id: Option<i64>,
    pub end_user: Option<String>,
    #[serde(skip)]
    pub tags_json: Option<String>,
    pub key_name: Option<String>,
    #[serde(skip)]
    pub last4: Option<String>,
    pub model_name: String,
    pub provider: String,
    pub status_code: i32,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub endpoint: String,
    #[serde(skip)]
    pub cost_nanos: i64,
    pub latency_ms: i64,
    pub stream: bool,
    pub error: Option<String>,
    /// Its key kept the request and the reply (reports leave it out).
    #[sqlx(default)]
    pub has_body: bool,
}

#[derive(Debug, Serialize)]
pub struct LogLine {
    #[serde(flatten)]
    pub entry: LogEntry,
    pub key_hint: Option<String>,
    pub tags: Vec<String>,
    pub total_tokens: i64,
    pub cost_usd: f64,
}

/// The log lines `user` sees, as filtered: `$1`…`$12`, bound by
/// [`bind_log_filter`]. `$12`, the body search, reads the full-text index:
/// its matches are found once, then joined to the logs by request id.
///
/// `$1` admin, `$2` the user, `$3` teams, `$4` keys, `$5` models, `$6`
/// status, `$7`..`$8` the times, `$9` customers, `$10` tags, `$11` the
/// `LIKE` search, `$12` the body search, `$13`…`$17` teams, keys, models,
/// customers and tags left out, `$18` labels, `$19` labels left out.
pub(crate) static LOG_FILTER: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let tagged = |n: u8, carrying: bool| {
        let not = if carrying { "" } else { "NOT " };
        format!(
            "(${n} IS NULL OR {not}EXISTS (SELECT 1 FROM {} WHERE value IN ({})))",
            crate::db::each("l.tags"),
            values(n, false)
        )
    };
    format!(
        "($1 OR l.user_id = $2
                OR l.team_id IN (SELECT team_id FROM team_members WHERE user_id = $2))
           AND {} AND {} AND {} AND {} AND {}
           AND ($6 = 0 OR ($6 = 1 AND l.status_code < 400) OR ($6 = 2 AND l.status_code >= 400))
           AND ($7 IS NULL OR l.created_at >= $7)
           AND ($8 IS NULL OR l.created_at <= $8)
           AND ($11 IS NULL OR LOWER(l.model_name) LIKE $11 ESCAPE '\\'
                OR LOWER(l.end_user) LIKE $11 ESCAPE '\\'
                OR LOWER(l.request_id) LIKE $11 ESCAPE '\\')
           AND ($12 IS NULL OR l.request_id IN ({}))
           AND {} AND {} AND {} AND {} AND {} AND {} AND {}",
        among("l.team_id", 3, true),
        among("l.key_id", 4, true),
        among("l.model_name", 5, false),
        among("l.end_user", 9, false),
        tagged(10, true),
        crate::db::BODIES_MATCHING_12,
        not_among("l.team_id", 13, true),
        not_among("l.key_id", 14, true),
        not_among("l.model_name", 15, false),
        not_among("l.end_user", 16, false),
        tagged(17, false),
        labelled("l.key_id", 18, true),
        labelled("l.key_id", 19, false),
    )
});

/// A log query, checked and ready to bind.
#[derive(Debug, Clone)]
pub(crate) struct LogFilter {
    pub is_admin: bool,
    pub user_id: i64,
    pub query: LogQuery,
    pub status: i32,
    pub search: Option<String>,
    /// `text` as the index's query, `None` when there is nothing to find.
    pub text: Option<String>,
}

impl LogFilter {
    pub async fn new(
        pool: &DbPool,
        user: &User,
        list: &ListQuery,
        query: &LogQuery,
    ) -> AppResult<Self> {
        for team_id in query.team_id.0.iter().chain(&query.exclude_team_id.0) {
            team_access(pool, user, *team_id).await?;
        }
        let status = match query.status.as_deref() {
            None | Some("") => 0,
            Some("success") => 1,
            Some("error") => 2,
            Some(other) => {
                return Err(AppError::Validation(format!(
                    "status must be 'success' or 'error', not '{other}'"
                )))
            }
        };
        let query = query.clone();
        Ok(Self {
            is_admin: user.is_admin(),
            user_id: user.id,
            text: query.text.as_deref().and_then(crate::db::text_query),
            query,
            status,
            search: list.search(),
        })
    }
}

/// Binds a [`LogFilter`] to `$1`…`$19` of [`LOG_FILTER`].
macro_rules! bind_log_filter {
    ($query:expr, $f:expr) => {
        $query
            .bind($f.is_admin)
            .bind($f.user_id)
            .bind($f.query.team_id.json())
            .bind($f.query.key_id.json())
            .bind($f.query.model.json())
            .bind($f.status)
            .bind($f.query.from)
            .bind($f.query.to)
            .bind($f.query.end_user.json())
            .bind($f.query.tag.json())
            .bind($f.search.as_deref())
            .bind($f.text.as_deref())
            .bind($f.query.exclude_team_id.json())
            .bind($f.query.exclude_key_id.json())
            .bind($f.query.exclude_model.json())
            .bind($f.query.exclude_end_user.json())
            .bind($f.query.exclude_tag.json())
            .bind($f.query.label_id.json())
            .bind($f.query.exclude_label_id.json())
    };
}
pub(crate) use bind_log_filter;

pub async fn logs(
    pool: &DbPool,
    user: &User,
    list: &ListQuery,
    query: &LogQuery,
) -> AppResult<Paged<LogLine>> {
    let filter = LogFilter::new(pool, user, list, query).await?;
    let order = list.order_by(
        &[
            ("created_at", "l.created_at"),
            ("cost", "l.cost_nanos"),
            ("tokens", "(l.prompt_tokens + l.completion_tokens)"),
            ("latency", "l.latency_ms"),
        ],
        "-created_at",
        "l.id",
    )?;
    // ponytail: COUNT(*) over the filtered logs on every page; the
    // dashboard's default 24 h window keeps it small. Keyset paging without
    // a total if logs reach the millions.
    let from = format!(
        "FROM request_logs l
         LEFT JOIN teams t ON t.id = l.team_id
         LEFT JOIN api_keys k ON k.id = l.key_id
         WHERE {}",
        *LOG_FILTER
    );
    let total: i64 = bind_log_filter!(
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) {from}"))),
        filter
    )
    .fetch_one(pool)
    .await?;
    let entries = bind_log_filter!(
        sqlx::query_as::<_, LogEntry>(sqlx::AssertSqlSafe(format!(
        "SELECT l.id, l.request_id, l.created_at, l.team_id, t.name AS team_name, l.user_id, l.key_id,
                l.model_id, l.end_user, l.tags AS tags_json,
                k.name AS key_name, k.last4, l.model_name, l.provider, l.status_code,
                l.prompt_tokens, l.completion_tokens, l.cached_tokens, l.cache_write_tokens,
                l.reasoning_tokens, l.endpoint, l.cost_nanos, l.latency_ms, l.stream, l.error,
                EXISTS (SELECT 1 FROM request_bodies b WHERE b.request_id = l.request_id) AS has_body
         {from}
         ORDER BY {order}
         LIMIT $20 OFFSET $21"
    ))),
        filter
    )
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    let lines = entries
        .into_iter()
        .map(|entry| LogLine {
            key_hint: entry.last4.as_ref().map(|l| format!("sk-...{l}")),
            tags: entry
                .tags_json
                .as_deref()
                .and_then(|t| serde_json::from_str(t).ok())
                .unwrap_or_default(),
            total_tokens: entry.prompt_tokens + entry.completion_tokens,
            cost_usd: nanos_to_usd(entry.cost_nanos),
            entry,
        })
        .collect();
    Ok(list.paged(lines, total))
}
