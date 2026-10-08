//! Usage reports to hand to a client: a PDF to read and an XLSX to work
//! with, in the reader's language and currency, marked up when asked.
//!
//! Everything is gathered here once, as integers; `pdf` and `xlsx` only lay
//! it out. Money stays nano-USD until a cell or a line of text needs it.

mod pdf;
mod text;
mod xlsx;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::gateway::pricing::Pricing;
use crate::models::llm_model::PricingSource;
use crate::models::user::{User, CURRENCIES};
use crate::services::models;
use crate::services::usage::{self, Day, Line, LogEntry, Totals, UsageQuery, UsageReport};

pub use text::Lang;
use text::T;

/// What a report can hold, in the order it holds them.
pub const SECTIONS: [&str; 13] = [
    "summary",
    "trend",
    "tokens",
    "teams",
    "keys",
    "people",
    "models",
    "customers",
    "tags",
    "daily",
    "lines",
    "requests",
    "rates",
];
/// Left out unless asked for: one row per request is long.
const OPT_IN: [&str; 1] = ["requests"];
/// Percent. Ten times the provider's cost is already a strange invoice.
const MAX_MARKUP: f64 = 1000.0;
const MAX_TEXT: usize = 200;
const MAX_NOTES: usize = 2000;
/// Requests a report lists at most: a PDF is read, a workbook filtered.
const PDF_REQUESTS: usize = 5_000;
const XLSX_REQUESTS: usize = 100_000;

/// `GET /api/usage/export`: the report's filters, what to put in it, and how.
#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// `pdf` or `xlsx`.
    pub format: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub team_id: Option<i64>,
    pub key_id: Option<i64>,
    pub model: Option<String>,
    pub person_id: Option<i64>,
    /// Comma-separated [`SECTIONS`]; every one but the requests when absent.
    pub sections: Option<String>,
    /// Percent added to the provider's cost.
    pub markup: Option<f64>,
    /// Shows the provider's cost and the margin beside the amount. Off by
    /// default: a document for the client should not carry the margin.
    #[serde(default)]
    pub show_cost: bool,
    pub client: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Xlsx,
}

/// A rendered report, ready to send.
pub struct File {
    pub name: String,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// The rates a model in the report is billed by today.
pub struct Rate {
    pub model: String,
    pub provider: String,
    pub source: PricingSource,
    pub pricing: Option<Pricing>,
}

/// Everything a report shows, gathered once.
pub struct Report {
    pub lang: Lang,
    pub currency: &'static str,
    /// Units of `currency` per US dollar.
    pub rate: f64,
    /// The markup in hundredths of a percent, so it applies in integers.
    pub markup_bp: i64,
    pub show_cost: bool,
    pub sections: Vec<&'static str>,
    pub client: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub author: String,
    /// The filters, as words: `Team: Acme`.
    pub scope: Vec<(T, String)>,
    pub generated: DateTime<Utc>,
    pub id: String,
    pub fingerprint: String,
    pub usage: UsageReport,
    /// The days just before, one per day of `usage.daily`, to compare with.
    pub previous_daily: Vec<Day>,
    pub lines: Vec<Line>,
    pub requests: Vec<LogEntry>,
    /// More requests matched than are listed.
    pub requests_capped: Option<usize>,
    pub rates: Vec<Rate>,
    filtered_by_model: bool,
    /// Breakdowns the filters already answer: one team filtered is one row
    /// in a table by team.
    pub narrowed: Vec<&'static str>,
}

impl Report {
    pub fn has(&self, section: &str) -> bool {
        self.sections.contains(&section)
    }

    pub fn marked_up(&self) -> bool {
        self.markup_bp > 0
    }

    /// Whether provider cost and margin are shown beside the amount.
    pub fn splits_cost(&self) -> bool {
        self.show_cost && self.marked_up()
    }

    /// What the client pays for `nanos` of provider cost, still nano-USD.
    pub fn billed_nanos(&self, nanos: i64) -> i64 {
        let billed = i128::from(nanos) * i128::from(10_000 + self.markup_bp) / 10_000;
        i64::try_from(billed).unwrap_or(i64::MAX)
    }

    /// Nano-USD in the report's currency.
    pub fn convert(&self, nanos: i64) -> f64 {
        nanos as f64 / 1e9 * self.rate
    }

    /// What the client pays, in the report's currency.
    pub fn amount(&self, nanos: i64) -> f64 {
        self.convert(self.billed_nanos(nanos))
    }

    /// The provider's cost, in the report's currency.
    pub fn cost(&self, nanos: i64) -> f64 {
        self.convert(nanos)
    }

    pub fn money(&self, value: f64) -> String {
        self.lang.money(self.currency, value)
    }

    pub fn t(&self, phrase: T) -> &'static str {
        self.lang.t(phrase)
    }

    pub fn period(&self) -> String {
        self.lang.period(self.usage.from, self.usage.to)
    }

    pub fn markup_percent(&self) -> f64 {
        self.markup_bp as f64 / 100.0
    }

    pub fn methodology(&self) -> Vec<String> {
        self.lang.methodology(
            &self.period(),
            self.currency,
            self.rate,
            &self.author,
            // Without the cost beside it, the markup is the reader's margin
            // to work out: it is not printed.
            if self.splits_cost() {
                self.markup_percent()
            } else {
                0.0
            },
            self.splits_cost(),
            &self.fingerprint,
            self.filtered_by_model,
        )
    }

    /// The name a line goes by under its team: the team, or whose personal
    /// key it was.
    pub fn team_of(&self, team: Option<&str>, owner: Option<&str>) -> String {
        match (team, owner) {
            (Some(team), _) => team.to_string(),
            (None, Some(owner)) => format!("{} ({owner})", self.t(T::Personal)),
            (None, None) => self.t(T::Personal).to_string(),
        }
    }

    pub fn key_of(&self, name: Option<&str>, id: i64) -> String {
        name.map(String::from)
            .unwrap_or_else(|| format!("{} #{id}", self.t(T::DeletedKey)))
    }

    pub fn source_name(&self, source: PricingSource) -> &'static str {
        self.t(match source {
            PricingSource::Catalog => T::SourceCatalog,
            PricingSource::Custom => T::SourceCustom,
            PricingSource::Free => T::SourceFree,
            PricingSource::None => T::SourceNone,
        })
    }

    /// A per-million-token rate (micro-USD) as the client sees it: marked up
    /// unless the provider's cost is shown apart.
    pub fn rate_value(&self, micros: i64) -> f64 {
        let nanos = micros.saturating_mul(1000);
        if self.splits_cost() {
            self.cost(nanos)
        } else {
            self.amount(nanos)
        }
    }

    pub fn file_name(&self, format: Format) -> String {
        let extension = match format {
            Format::Pdf => "pdf",
            Format::Xlsx => "xlsx",
        };
        format!(
            "llmtrack-usage-{}-{}.{extension}",
            self.usage.from, self.usage.to
        )
    }
}

/// Builds the report `query` asks for and renders it.
pub async fn export(
    state: &AppState,
    user: &User,
    query: &ExportQuery,
    accept_language: Option<&str>,
) -> AppResult<File> {
    let (report, format) = gather(state, user, query, accept_language).await?;
    let name = report.file_name(format);
    // Laying out pages is CPU work: keep it off the threads that answer.
    let bytes = tokio::task::spawn_blocking(move || match format {
        Format::Pdf => pdf::render(&report),
        Format::Xlsx => xlsx::render(&report),
    })
    .await
    .map_err(|e| AppError::Internal(format!("report task: {e}")))??;
    Ok(File {
        name,
        content_type: match format {
            Format::Pdf => "application/pdf",
            Format::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        },
        bytes,
    })
}

fn invalid(field: &str, message: impl Into<String>) -> AppError {
    AppError::Validation(message.into()).with_field(field, FieldErrorCode::Invalid)
}

/// Trimmed, empty as absent, refused past `max` characters.
fn optional_text(value: Option<&str>, field: &str, max: usize) -> AppResult<Option<String>> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) if v.chars().count() > max => Err(invalid(
            field,
            format!("{field} must be at most {max} characters"),
        )),
        other => Ok(other.map(String::from)),
    }
}

fn sections(asked: Option<&str>) -> AppResult<Vec<&'static str>> {
    let Some(asked) = asked.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(SECTIONS
            .into_iter()
            .filter(|s| !OPT_IN.contains(s))
            .collect());
    };
    let mut wanted = Vec::new();
    for part in asked.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let Some(section) = SECTIONS.iter().find(|s| **s == part) else {
            return Err(invalid(
                "sections",
                format!("sections must be among {}", SECTIONS.join(", ")),
            ));
        };
        wanted.push(*section);
    }
    // In the report's own order, whatever order they were asked in.
    Ok(SECTIONS
        .into_iter()
        .filter(|s| wanted.contains(s))
        .collect())
}

async fn gather(
    state: &AppState,
    user: &User,
    query: &ExportQuery,
    accept_language: Option<&str>,
) -> AppResult<(Report, Format)> {
    let format = match query.format.as_str() {
        "pdf" => Format::Pdf,
        "xlsx" => Format::Xlsx,
        _ => return Err(invalid("format", "format must be 'pdf' or 'xlsx'")),
    };
    let markup = query.markup.unwrap_or(0.0);
    if !markup.is_finite() || !(0.0..=MAX_MARKUP).contains(&markup) {
        return Err(invalid(
            "markup",
            format!("markup must be a percentage between 0 and {MAX_MARKUP}"),
        ));
    }
    let sections = sections(query.sections.as_deref())?;
    let client = optional_text(query.client.as_deref(), "client", MAX_TEXT)?;
    let reference = optional_text(query.reference.as_deref(), "reference", MAX_TEXT)?;
    let notes = optional_text(query.notes.as_deref(), "notes", MAX_NOTES)?;

    let usage_query = UsageQuery {
        from: query.from,
        to: query.to,
        team_id: query.team_id,
        key_id: query.key_id,
        model: query.model.clone(),
        group_by: None,
        person_id: query.person_id,
    };
    // Checks the range and the team before anything else is read.
    let usage = usage::report(&state.pool, user, &usage_query).await?;
    let (from, to) = (usage.from, usage.to);
    let days = (to - from).num_days() + 1;

    let previous_daily = if sections.contains(&"trend") {
        let previous = UsageQuery {
            from: Some(from - chrono::Duration::days(days)),
            to: Some(from - chrono::Duration::days(1)),
            ..usage_query.clone()
        };
        usage::report(&state.pool, user, &previous).await?.daily
    } else {
        Vec::new()
    };

    let lines = usage::lines(&state.pool, user, &usage_query, from, to).await?;

    let (requests, requests_capped) = if sections.contains(&"requests") {
        let cap = match format {
            Format::Pdf => PDF_REQUESTS,
            Format::Xlsx => XLSX_REQUESTS,
        };
        let mut requests =
            usage::requests(&state.pool, user, &usage_query, from, to, cap as i64 + 1).await?;
        let capped = (requests.len() > cap).then_some(cap);
        requests.truncate(cap);
        (requests, capped)
    } else {
        (Vec::new(), None)
    };

    let rates = rates(state, &usage).await?;

    let lang = Lang::pick(user.language.as_deref(), accept_language);
    // The dashboard's rule: without a usable rate there is nothing to
    // convert with, so dollars it is.
    let (currency, rate) = match (user.currency.as_deref(), user.currency_rate) {
        (Some(c), Some(rate)) if rate.is_finite() && rate > 0.0 => {
            match CURRENCIES.iter().find(|known| **known == c) {
                Some(known) => (*known, rate),
                None => ("USD", 1.0),
            }
        }
        _ => ("USD", 1.0),
    };

    // The team was checked by the report; keys and people are named from
    // the rows the user may see.
    let team_name: Option<String> = match query.team_id {
        Some(id) => {
            sqlx::query_scalar("SELECT name FROM teams WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.pool)
                .await?
        }
        None => None,
    };
    let scope = describe_scope(lang, &usage, team_name.clone(), &usage_query);
    // Filtered to one team, the team is who the report is for.
    let client = client.or(team_name);
    let narrowed = [
        ("teams", query.team_id.is_some()),
        ("keys", query.key_id.is_some()),
        ("people", query.person_id.is_some()),
        (
            "models",
            query.model.as_deref().is_some_and(|m| !m.is_empty()),
        ),
    ]
    .into_iter()
    .filter_map(|(section, filtered)| filtered.then_some(section))
    .collect();

    let fingerprint = fingerprint(&lines);
    let generated = Utc::now();
    let id = format!(
        "LT-{}-{}",
        generated.format("%Y%m%d"),
        fingerprint[..8].to_ascii_uppercase()
    );

    Ok((
        Report {
            lang,
            currency,
            rate: if currency == "USD" { 1.0 } else { rate },
            markup_bp: (markup * 100.0).round() as i64,
            show_cost: query.show_cost,
            sections,
            client,
            reference,
            notes,
            author: user.name.clone().unwrap_or_else(|| user.email.clone()),
            scope,
            generated,
            id,
            fingerprint,
            usage,
            previous_daily,
            lines,
            requests,
            requests_capped,
            rates,
            narrowed,
            filtered_by_model: query.model.as_deref().is_some_and(|m| !m.is_empty())
                || query.person_id.is_some(),
        },
        format,
    ))
}

fn describe_scope(
    lang: Lang,
    usage: &UsageReport,
    team: Option<String>,
    query: &UsageQuery,
) -> Vec<(T, String)> {
    let named = |name: Option<String>, id: Option<i64>| {
        id.map(|id| name.unwrap_or_else(|| format!("#{id}")))
    };
    let key = query.key_id.and_then(|id| {
        usage
            .by_key
            .iter()
            .find(|k| k.key_id == id)
            .map(|k| format!("{} ({})", k.key_name, k.key_hint))
    });
    let person = query.person_id.and_then(|id| {
        usage
            .by_person
            .iter()
            .find(|p| p.person_id == id)
            .map(|p| p.person_name.clone())
    });
    let mut out = Vec::new();
    if let Some(team) = named(team, query.team_id) {
        out.push((T::Team, team));
    }
    if let Some(key) = named(key, query.key_id) {
        out.push((T::Key, key));
    }
    if let Some(person) = named(person, query.person_id) {
        out.push((T::Person, person));
    }
    if let Some(model) = query.model.as_deref().filter(|m| !m.is_empty()) {
        out.push((T::Model, model.to_string()));
    }
    if out.is_empty() {
        out.push((T::Scope, lang.t(T::Everything).to_string()));
    }
    out
}

/// SHA-256 over the detail lines, written out in a fixed order.
fn fingerprint(lines: &[Line]) -> String {
    let mut hash = Sha256::new();
    for l in lines {
        hash.update(
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                l.day,
                l.key_id,
                l.model_name,
                l.requests,
                l.failed_requests,
                l.prompt_tokens,
                l.completion_tokens,
                l.cached_tokens,
                l.cache_write_tokens,
                l.reasoning_tokens,
                l.cost_nanos
            )
            .as_bytes(),
        );
    }
    hex::encode(hash.finalize())
}

/// Today's rates of every model the report bills, by its first deployment.
async fn rates(state: &AppState, usage: &UsageReport) -> AppResult<Vec<Rate>> {
    let deployments = models::list(&state.pool).await?;
    let catalog = state.gateway.catalog();
    Ok(usage
        .by_model
        .iter()
        .map(|m| {
            let deployment = deployments
                .iter()
                .filter(|d| d.name == m.model_name)
                .max_by_key(|d| d.is_active);
            match deployment {
                Some(d) => {
                    let (pricing, source) = models::effective_pricing(d, &catalog);
                    Rate {
                        model: m.model_name.clone(),
                        provider: d.provider.clone(),
                        source,
                        pricing,
                    }
                }
                None => Rate {
                    model: m.model_name.clone(),
                    provider: String::new(),
                    source: PricingSource::None,
                    pricing: None,
                },
            }
        })
        .collect())
}

/// Totals of a breakdown row, with the name it is shown under.
pub struct Group {
    pub name: String,
    /// A second column, when the breakdown has one (a key's team).
    pub detail: Option<String>,
    pub totals: Totals,
}

impl Report {
    /// The breakdowns a section shows, by section name.
    pub fn groups(&self, section: &str) -> Vec<Group> {
        let u = &self.usage;
        let group = |name: String, detail: Option<String>, totals: &Totals| Group {
            name,
            detail,
            totals: *totals,
        };
        match section {
            "teams" => u
                .by_team
                .iter()
                .map(|r| {
                    let name = match (&r.team_name, r.team_id) {
                        (Some(name), _) => name.clone(),
                        (None, None) => self.t(T::Personal).to_string(),
                        (None, Some(id)) => format!("{} #{id}", self.t(T::DeletedTeam)),
                    };
                    group(name, None, &r.totals)
                })
                .collect(),
            "keys" => u
                .by_key
                .iter()
                .map(|r| {
                    group(
                        format!("{} ({})", r.key_name, r.key_hint),
                        Some(self.team_of(r.team_name.as_deref(), None)),
                        &r.totals,
                    )
                })
                .collect(),
            "people" => u
                .by_person
                .iter()
                .map(|r| group(r.person_name.clone(), r.team_name.clone(), &r.totals))
                .collect(),
            "models" => u
                .by_model
                .iter()
                .map(|r| {
                    let provider = self
                        .rates
                        .iter()
                        .find(|rate| rate.model == r.model_name)
                        .map(|rate| rate.provider.clone())
                        .filter(|p| !p.is_empty());
                    group(r.model_name.clone(), provider, &r.totals)
                })
                .collect(),
            "customers" => u
                .by_end_user
                .iter()
                .map(|r| group(r.end_user.clone(), None, &r.totals))
                .collect(),
            "tags" => u
                .by_tag
                .iter()
                .map(|r| group(r.tag.clone(), None, &r.totals))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The breakdown sections present, with their titles, sheet names and
    /// the heading of their second column.
    pub fn breakdowns(&self) -> Vec<(&'static str, T, T, Option<T>)> {
        [
            ("teams", T::ByTeam, T::SheetTeams, None),
            ("keys", T::ByKey, T::SheetKeys, Some(T::Team)),
            ("people", T::ByPerson, T::SheetPeople, Some(T::Team)),
            ("models", T::ByModel, T::SheetModels, Some(T::Provider)),
            ("customers", T::ByCustomer, T::SheetCustomers, None),
            ("tags", T::ByTag, T::SheetTags, None),
        ]
        .into_iter()
        .filter(|(s, ..)| self.has(s) && !self.narrowed.contains(s))
        .collect()
    }
}

/// Input tokens not served from or written to the cache.
pub fn uncached_input(t: &Totals) -> i64 {
    (t.prompt_tokens - t.cached_tokens - t.cache_write_tokens).max(0)
}

/// Output tokens that were not reasoning.
pub fn plain_output(t: &Totals) -> i64 {
    (t.completion_tokens - t.reasoning_tokens).max(0)
}

/// Average latency in milliseconds, if anything was timed.
pub fn avg_latency(t: &Totals) -> Option<f64> {
    (t.requests > 0).then(|| t.latency_ms as f64 / t.requests as f64)
}

/// The change from `previous` to `current`, if there was a previous.
pub fn change(current: f64, previous: f64) -> Option<f64> {
    (previous > 0.0).then(|| current / previous - 1.0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::gateway::pricing::TokenRates;
    use crate::services::usage::{KeyUsage, ModelUsage, TeamUsage};

    fn totals(n: i64) -> Totals {
        Totals {
            cost_usd: 0.0,
            cost_nanos: n * 1_234_567,
            requests: n,
            failed_requests: n / 20,
            prompt_tokens: n * 900,
            completion_tokens: n * 300,
            cached_tokens: n * 200,
            cache_write_tokens: n * 50,
            reasoning_tokens: n * 40,
            latency_ms: n * 850,
        }
    }

    /// A report over 45 days with `count` detail lines, for the renderers.
    pub(crate) fn sample(lang: Lang, count: usize, show_cost: bool) -> Report {
        let from = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        let day = |i: i64| (from + chrono::Duration::days(i)).to_string();
        let daily: Vec<Day> = (0..45)
            .map(|i| Day {
                day: day(i),
                totals: totals((i * 37) % 90 + 5),
            })
            .collect();
        let previous_daily = (0..45)
            .map(|i| Day {
                day: day(i - 45),
                totals: totals((i * 13) % 70 + 3),
            })
            .collect();
        let models = [
            "gpt-4o",
            "claude-sonnet-4-5",
            "gemini-2.5-pro",
            "llama-3.3-70b",
        ];
        let teams = [
            "Acme Corporation — Research & Development",
            "Globex",
            "Initech",
        ];
        let lines: Vec<Line> = (0..count)
            .map(|i| {
                let t = totals((i as i64 * 7) % 40 + 1);
                Line {
                    day: day((i / 12) as i64 % 45),
                    key_id: (i % 9) as i64,
                    key_name: Some(format!("backend-service-{}", i % 9)),
                    last4: Some("a1b2".into()),
                    team_name: (i % 4 != 0).then(|| teams[i % 3].to_string()),
                    owner: Some("ana@example.com".into()),
                    person_name: None,
                    model_name: models[i % 4].into(),
                    requests: t.requests,
                    failed_requests: t.failed_requests,
                    prompt_tokens: t.prompt_tokens,
                    completion_tokens: t.completion_tokens,
                    cached_tokens: t.cached_tokens,
                    cache_write_tokens: t.cache_write_tokens,
                    reasoning_tokens: t.reasoning_tokens,
                    cost_nanos: t.cost_nanos,
                    latency_ms: t.latency_ms,
                }
            })
            .collect();
        let mut sum = Totals::default();
        for d in &daily {
            let t = d.totals;
            sum.cost_nanos += t.cost_nanos;
            sum.requests += t.requests;
            sum.failed_requests += t.failed_requests;
            sum.prompt_tokens += t.prompt_tokens;
            sum.completion_tokens += t.completion_tokens;
            sum.cached_tokens += t.cached_tokens;
            sum.cache_write_tokens += t.cache_write_tokens;
            sum.reasoning_tokens += t.reasoning_tokens;
            sum.latency_ms += t.latency_ms;
        }
        let requests = (0..40)
            .map(|i| LogEntry {
                id: i,
                request_id: format!("chatcmpl-{i:08}-5f2c9a1e"),
                created_at: from.and_hms_opt(9, 30, 0).unwrap().and_utc(),
                team_id: Some(1),
                team_name: Some("Globex".into()),
                user_id: None,
                key_id: 1,
                model_id: Some(1),
                end_user: Some("customer-42".into()),
                tags_json: Some(r#"["prod"]"#.into()),
                key_name: Some("backend".into()),
                last4: Some("a1b2".into()),
                model_name: "gpt-4o".into(),
                provider: "openai".into(),
                status_code: if i % 9 == 0 { 429 } else { 200 },
                prompt_tokens: 1200,
                completion_tokens: 300,
                cached_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
                endpoint: "chat/completions".into(),
                cost_nanos: 6_000_000,
                latency_ms: 812,
                stream: false,
                error: None,
            })
            .collect();
        let usage = UsageReport {
            from,
            to,
            totals: sum,
            previous: totals(1500),
            series: Vec::new(),
            daily,
            by_team: teams
                .iter()
                .enumerate()
                .map(|(i, t)| TeamUsage {
                    team_id: Some(i as i64),
                    team_name: Some(t.to_string()),
                    totals: totals(900 - i as i64 * 250),
                })
                .collect(),
            by_key: (0..12)
                .map(|i| KeyUsage {
                    key_id: i,
                    key_name: format!("backend-service-{i}"),
                    key_hint: "sk-...a1b2".into(),
                    team_name: Some(teams[i as usize % 3].into()),
                    totals: totals(400 - i * 30),
                })
                .collect(),
            by_person: Vec::new(),
            by_model: models
                .iter()
                .enumerate()
                .map(|(i, m)| ModelUsage {
                    model_name: m.to_string(),
                    totals: totals(800 - i as i64 * 180),
                })
                .collect(),
            by_end_user: Vec::new(),
            by_tag: Vec::new(),
        };
        let rates = models
            .iter()
            .map(|m| Rate {
                model: m.to_string(),
                provider: "openai".into(),
                source: PricingSource::Catalog,
                pricing: Some(Pricing {
                    tokens: TokenRates {
                        input: 2_500_000,
                        output: 10_000_000,
                        cache_read: Some(1_250_000),
                        cache_write: None,
                        cache_write_1h: None,
                        reasoning: None,
                    },
                    ..Pricing::default()
                }),
            })
            .collect();
        let fingerprint = fingerprint(&lines);
        Report {
            lang,
            currency: "EUR",
            rate: 0.92,
            markup_bp: 1500,
            show_cost,
            sections: SECTIONS.to_vec(),
            client: Some("Initech Iberia, S.L.".into()),
            reference: Some("PO-2026-118".into()),
            notes: Some("Consumo de septiembre.\nCondiciones según contrato marco.".into()),
            author: "Ana Pérez".into(),
            scope: vec![(T::Scope, lang.t(T::Everything).into())],
            generated: Utc::now(),
            id: format!("LT-20261008-{}", fingerprint[..8].to_ascii_uppercase()),
            fingerprint,
            usage,
            previous_daily,
            lines,
            requests,
            requests_capped: Some(40),
            rates,
            filtered_by_model: false,
            narrowed: Vec::new(),
        }
    }

    #[test]
    fn sections_default_to_all_but_requests_and_keep_their_order() {
        let all = sections(None).unwrap();
        assert!(!all.contains(&"requests"));
        assert_eq!(all.len(), SECTIONS.len() - 1);
        assert_eq!(
            sections(Some("lines, summary")).unwrap(),
            ["summary", "lines"]
        );
        assert!(sections(Some("summary,nope")).is_err());
    }

    #[test]
    fn text_options_are_trimmed_and_bounded() {
        assert_eq!(optional_text(Some("  "), "client", 5).unwrap(), None);
        assert_eq!(
            optional_text(Some(" Initech "), "client", 10).unwrap(),
            Some("Initech".into())
        );
        assert!(optional_text(Some("ñññññ ñ"), "client", 5).is_err());
    }
}
