//! Spend reports. Totals and breakdowns read the daily rollup; the request
//! log is only paged through, never aggregated.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::DbPool;
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::list::{ListQuery, Paged};
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
    pub team_id: Option<i64>,
    pub key_id: Option<i64>,
    /// Only this public model name. Customers and tags are rolled up without
    /// the model, so with it set their breakdowns come back empty.
    pub model: Option<String>,
    /// What `series` splits by: `model` (the default), `team`, `key`,
    /// `person`, `end_user` or `tag`.
    pub group_by: Option<String>,
    /// Only the keys' spend while assigned to this person. Like the model,
    /// customers and tags do not carry it.
    pub person_id: Option<i64>,
    /// Only the keys that carry this label now, all their spend included.
    pub label_id: Option<i64>,
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

/// Rows of the keys carrying label `$9`, when one is asked for.
const LABELLED: &str = "(CAST($9 AS BIGINT) IS NULL
    OR u.key_id IN (SELECT key_id FROM key_labels WHERE label_id = $9))";

/// Rows `user` may see: everything for an admin; their teams' and their
/// personal keys' otherwise.
const VISIBLE: &str = "($3 OR u.user_id = $4
    OR u.team_id IN (SELECT team_id FROM team_members WHERE user_id = $4))";

/// The rollup rows of `table` a report covers. Binds: `$1`..`$2` the days,
/// `$3` admin, `$4` the user, `$5` team, `$6` key, `$7` model, `$8` person,
/// `$9` label.
///
/// Customers and tags are rolled up without the model: a model filter
/// matches none of their rows rather than all of them.
fn filters(table: &str) -> String {
    let (model, person) = if table == "usage_daily" {
        (
            "(CAST($7 AS TEXT) IS NULL OR u.model_name = $7)",
            "(CAST($8 AS BIGINT) IS NULL OR u.person_id = $8)",
        )
    } else {
        ("CAST($7 AS TEXT) IS NULL", "CAST($8 AS BIGINT) IS NULL")
    };
    format!(
        "u.day >= $1 AND u.day <= $2 AND {VISIBLE}
         AND ($5 IS NULL OR u.team_id = $5) AND ($6 IS NULL OR u.key_id = $6)
         AND {model} AND {person} AND {LABELLED}"
    )
}

pub async fn report(pool: &DbPool, user: &User, query: &UsageQuery) -> AppResult<UsageReport> {
    if let Some(team_id) = query.team_id {
        team_access(pool, user, team_id).await?;
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
    let model = query.model.as_deref().filter(|m| !m.is_empty());

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
        sqlx::query_as::<_, Row>(sqlx::AssertSqlSafe(sql))
            .bind(from.to_string())
            .bind(to.to_string())
            .bind(user.is_admin())
            .bind(user.id)
            .bind(query.team_id)
            .bind(query.key_id)
            .bind(model)
            .bind(query.person_id)
            .bind(query.label_id)
            .fetch_all(pool)
    };

    let previous_from = from - chrono::Duration::days(days);
    let previous_to = from - chrono::Duration::days(1);
    let previous = sqlx::query_as::<_, Sums>(sqlx::AssertSqlSafe(format!(
        "SELECT {SUMS} FROM usage_daily u WHERE {}",
        filters("usage_daily")
    )))
    .bind(previous_from.to_string())
    .bind(previous_to.to_string())
    .bind(user.is_admin())
    .bind(user.id)
    .bind(query.team_id)
    .bind(query.key_id)
    .bind(model)
    .bind(query.person_id)
    .bind(query.label_id)
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
    let series = sqlx::query_as::<_, SeriesRow>(sqlx::AssertSqlSafe(format!(
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
    .bind(user.id)
    .bind(query.team_id)
    .bind(query.key_id)
    .bind(model)
    .bind(query.person_id)
    .bind(query.label_id)
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
    Ok(sqlx::query_as::<_, Line>(sqlx::AssertSqlSafe(format!(
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
    .bind(user.id)
    .bind(query.team_id)
    .bind(query.key_id)
    .bind(query.model.as_deref().filter(|m| !m.is_empty()))
    .bind(query.person_id)
    .bind(query.label_id)
    .fetch_all(pool)
    .await?)
}

/// Up to `limit` requests of `from..=to` (UTC days), oldest first, under the
/// report's filters. Requests do not carry the person, so a person filter
/// matches none.
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
    Ok(sqlx::query_as::<_, LogEntry>(
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
           AND ($5 IS NULL OR l.team_id = $5) AND ($6 IS NULL OR l.key_id = $6)
           AND (CAST($7 AS TEXT) IS NULL OR l.model_name = $7)
           AND CAST($8 AS BIGINT) IS NULL
           AND (CAST($9 AS BIGINT) IS NULL
                OR l.key_id IN (SELECT key_id FROM key_labels WHERE label_id = $9))
         ORDER BY l.created_at, l.id
         LIMIT $10",
    )
    .bind(start)
    .bind(end)
    .bind(user.is_admin())
    .bind(user.id)
    .bind(query.team_id)
    .bind(query.key_id)
    .bind(query.model.as_deref().filter(|m| !m.is_empty()))
    .bind(query.person_id)
    .bind(query.label_id)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogQuery {
    pub team_id: Option<i64>,
    pub key_id: Option<i64>,
    pub model: Option<String>,
    /// `success` or `error`.
    pub status: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub end_user: Option<String>,
    pub tag: Option<String>,
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

/// The log lines `user` sees, as filtered: `$1`…`$11`, bound by
/// [`bind_log_filter`].
pub(crate) const LOG_FILTER: &str = "
           ($1 OR l.user_id = $2
                OR l.team_id IN (SELECT team_id FROM team_members WHERE user_id = $2))
           AND ($3 IS NULL OR l.team_id = $3)
           AND ($4 IS NULL OR l.key_id = $4)
           AND ($5 IS NULL OR l.model_name = $5)
           AND ($6 = 0 OR ($6 = 1 AND l.status_code < 400) OR ($6 = 2 AND l.status_code >= 400))
           AND ($7 IS NULL OR l.created_at >= $7)
           AND ($8 IS NULL OR l.created_at <= $8)
           AND ($9 IS NULL OR l.end_user = $9)
           AND ($10 IS NULL OR l.tags LIKE $10 ESCAPE '\\')
           AND ($11 IS NULL OR LOWER(l.model_name) LIKE $11 ESCAPE '\\'
                OR LOWER(l.end_user) LIKE $11 ESCAPE '\\'
                OR LOWER(l.request_id) LIKE $11 ESCAPE '\\')";

/// A log query, checked and ready to bind.
#[derive(Debug, Clone)]
pub(crate) struct LogFilter {
    pub is_admin: bool,
    pub user_id: i64,
    pub query: LogQuery,
    pub status: i32,
    pub tag: Option<String>,
    pub search: Option<String>,
}

impl LogFilter {
    pub async fn new(
        pool: &DbPool,
        user: &User,
        list: &ListQuery,
        query: &LogQuery,
    ) -> AppResult<Self> {
        if let Some(team_id) = query.team_id {
            team_access(pool, user, team_id).await?;
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
        let mut query = query.clone();
        query.model = query.model.filter(|m| !m.is_empty());
        query.end_user = query.end_user.filter(|u| !u.is_empty());
        Ok(Self {
            is_admin: user.is_admin(),
            user_id: user.id,
            tag: query
                .tag
                .as_deref()
                .filter(|t| !t.is_empty())
                .map(tag_pattern),
            query,
            status,
            search: list.search(),
        })
    }
}

/// Binds a [`LogFilter`] to `$1`…`$11` of [`LOG_FILTER`].
macro_rules! bind_log_filter {
    ($query:expr, $f:expr) => {
        $query
            .bind($f.is_admin)
            .bind($f.user_id)
            .bind($f.query.team_id)
            .bind($f.query.key_id)
            .bind($f.query.model.as_deref())
            .bind($f.status)
            .bind($f.query.from)
            .bind($f.query.to)
            .bind($f.query.end_user.as_deref())
            .bind($f.tag.as_deref())
            .bind($f.search.as_deref())
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
         WHERE {LOG_FILTER}"
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
         LIMIT $12 OFFSET $13"
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

/// A LIKE pattern matching `tag` as one element of the stored JSON array.
fn tag_pattern(tag: &str) -> String {
    let quoted = serde_json::to_string(tag).unwrap_or_default();
    let escaped = quoted
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tag_pattern_matches_the_whole_quoted_tag_only() {
        assert_eq!(super::tag_pattern("prod"), "%\"prod\"%");
        assert_eq!(super::tag_pattern("50%_off"), "%\"50\\%\\_off\"%");
    }
}
