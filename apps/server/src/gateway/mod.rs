//! The proxy: authenticates virtual keys, routes to providers, meters spend.
//!
//! The hot path touches the database only on a cache miss. Keys and models
//! are cached in memory until something changes them ([`Gateway::invalidate`]),
//! spend is counted in memory as requests finish, and usage reaches the
//! database in batches off the request path ([`usage`]).

pub mod anthropic;
pub mod capture;
pub mod catalog;
pub mod forward;
pub mod messages;
pub mod pricing;
pub mod providers;
pub mod responses;
pub mod router;
pub mod sse;
pub mod usage;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::crypto::{hash_key, SecretBox};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::limits::next_reset;
use catalog::Catalog;
use pricing::Pricing;
use providers::Wire;
use usage::Message;

pub use pricing::Usage;

impl Usage {
    /// OpenAI's `usage` (chat, completions, embeddings), including the
    /// cached and reasoning breakdowns and DeepSeek's cache-hit field.
    pub fn from_openai(usage: &Value) -> Option<Self> {
        let prompt_tokens = usage["prompt_tokens"].as_i64()?;
        Some(Self {
            prompt_tokens,
            completion_tokens: usage["completion_tokens"].as_i64().unwrap_or(0),
            cached_tokens: usage["prompt_tokens_details"]["cached_tokens"]
                .as_i64()
                .or_else(|| usage["prompt_cache_hit_tokens"].as_i64())
                .unwrap_or(0),
            reasoning_tokens: usage["completion_tokens_details"]["reasoning_tokens"]
                .as_i64()
                .unwrap_or(0),
            ..Self::default()
        })
    }

    /// The Responses API's `usage` (`input_tokens`, `output_tokens`).
    pub fn from_responses(usage: &Value) -> Option<Self> {
        let prompt_tokens = usage["input_tokens"].as_i64()?;
        Some(Self {
            prompt_tokens,
            completion_tokens: usage["output_tokens"].as_i64().unwrap_or(0),
            cached_tokens: usage["input_tokens_details"]["cached_tokens"]
                .as_i64()
                .unwrap_or(0),
            reasoning_tokens: usage["output_tokens_details"]["reasoning_tokens"]
                .as_i64()
                .unwrap_or(0),
            ..Self::default()
        })
    }

    /// Anthropic's `usage`, whose `input_tokens` excludes the cache: the
    /// cache is added back so `prompt_tokens` means what OpenAI means.
    pub fn from_anthropic(usage: &Value) -> Self {
        let read = usage["cache_read_input_tokens"].as_i64().unwrap_or(0);
        let written = usage["cache_creation_input_tokens"].as_i64().unwrap_or(0);
        Self {
            prompt_tokens: usage["input_tokens"].as_i64().unwrap_or(0) + read + written,
            completion_tokens: usage["output_tokens"].as_i64().unwrap_or(0),
            cached_tokens: read,
            cache_write_tokens: written,
            cache_write_1h_tokens: usage["cache_creation"]["ephemeral_1h_input_tokens"]
                .as_i64()
                .unwrap_or(0),
            ..Self::default()
        }
    }

    /// OpenAI's `usage` object for a translated response.
    pub fn to_json(self) -> Value {
        json!({
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "total_tokens": self.prompt_tokens + self.completion_tokens,
            "prompt_tokens_details": { "cached_tokens": self.cached_tokens },
            "completion_tokens_details": { "reasoning_tokens": self.reasoning_tokens },
        })
    }
}

/// One deployment of a model name, with its provider secret already
/// decrypted.
#[derive(Debug)]
pub struct Route {
    /// The model row's id (`model_id`).
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub wire: Wire,
    /// Whether `stream_options.include_usage` may be sent.
    pub stream_usage: bool,
    /// Whether the provider serves `/responses` itself.
    pub native_responses: bool,
    pub upstream_model: String,
    pub api_base: String,
    pub api_version: Option<String>,
    pub api_key: Option<String>,
    /// What requests are billed by; zero rates when the model is unpriced.
    pub pricing: Pricing,
    /// The catalog's facts about the provider model, when it is listed.
    pub max_output_tokens: Option<i64>,
    pub supports_native_structured_output: bool,
    pub created_at: DateTime<Utc>,
    /// Share of its name's traffic; see [`router`].
    pub weight: Option<i64>,
}

/// Every active deployment, grouped by model name, and how to route them.
#[derive(Debug, Default)]
pub struct RouteTable {
    pub groups: HashMap<String, Vec<Arc<Route>>>,
    pub settings: router::RouterSettings,
}

pub type Routes = Arc<RouteTable>;

/// Who a request is for: the key, plus the end user and tags it named,
/// which spend is also reported by.
#[derive(Debug)]
pub struct Caller {
    pub key: Arc<KeyContext>,
    pub end_user: Option<String>,
    pub tags: Vec<String>,
    /// Held for as long as the request runs.
    pub slot: Slot,
}

/// Requests and tokens used in the current minute, by a key or a team.
// ponytail: fixed one-minute windows per process; a sliding window or a
// shared store (Redis) when limits must hold across replicas.
#[derive(Debug, Default)]
pub(crate) struct Window {
    minute: Mutex<[i64; 3]>,
    /// Requests admitted and not finished yet.
    in_flight: AtomicI64,
}

impl Window {
    fn used(&self, minute: i64) -> (i64, i64) {
        let w = self.minute.lock().expect("poisoned");
        if w[0] == minute {
            (w[1], w[2])
        } else {
            (0, 0)
        }
    }

    fn add(&self, minute: i64, requests: i64, tokens: i64) {
        let mut w = self.minute.lock().expect("poisoned");
        if w[0] != minute {
            *w = [minute, 0, 0];
        }
        w[1] += requests;
        w[2] += tokens;
    }
}

/// A request's place in the in-flight counts of its key and team, given
/// back when it is dropped: when the response, streams included, is done.
#[derive(Debug, Default)]
pub struct Slot(Vec<Arc<Window>>);

impl Drop for Slot {
    fn drop(&mut self) {
        for window in &self.0 {
            window.in_flight.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

fn minute(now: DateTime<Utc>) -> i64 {
    now.timestamp().div_euclid(60)
}

/// What a key or a team may spend and how fast. `spend` and `window` are
/// shared with every other context of the same key or team, and kept across
/// cache invalidations: they are the live figures.
#[derive(Debug)]
pub(crate) struct Limits {
    who: &'static str,
    id: i64,
    budget: Option<i64>,
    period: Option<String>,
    reset_at: Option<DateTime<Utc>>,
    rpm: Option<i64>,
    tpm: Option<i64>,
    /// `max_parallel_requests`.
    parallel: Option<i64>,
    pub(crate) spend: Arc<AtomicI64>,
    window: Arc<Window>,
}

impl Limits {
    fn due(&self, now: DateTime<Utc>) -> bool {
        self.reset_at.is_some_and(|at| at <= now)
    }

    fn check(&self, minute: i64) -> AppResult<()> {
        let who = self.who;
        if self
            .budget
            .is_some_and(|budget| self.spend.load(Ordering::Relaxed) >= budget)
        {
            return Err(AppError::BudgetExceeded(format!(
                "this {who} has spent its budget"
            )));
        }
        let (requests, tokens) = self.window.used(minute);
        if self.rpm.is_some_and(|rpm| requests >= rpm) {
            return Err(AppError::RateLimited(format!(
                "this {who} has reached its limit of requests per minute"
            )));
        }
        if self.tpm.is_some_and(|tpm| tokens >= tpm) {
            return Err(AppError::RateLimited(format!(
                "this {who} has reached its limit of tokens per minute"
            )));
        }
        Ok(())
    }
}

/// Everything the gateway needs to admit a request for a key.
#[derive(Debug)]
pub struct KeyContext {
    pub key_id: i64,
    /// A team key bills to its team, a personal key to its user.
    pub team_id: Option<i64>,
    pub user_id: Option<i64>,
    /// Keep each request's body and reply.
    pub log_bodies: bool,
    blocked: bool,
    expires_at: Option<DateTime<Utc>>,
    /// Model names this key may call; `None` means every model.
    allowed: Option<HashSet<String>>,
    pub(crate) key: Limits,
    pub(crate) team: Option<Limits>,
}

impl KeyContext {
    pub fn may_call(&self, model: &str) -> bool {
        self.allowed
            .as_ref()
            .is_none_or(|names| names.contains(model))
    }

    fn limits(&self) -> impl Iterator<Item = &Limits> {
        std::iter::once(&self.key).chain(&self.team)
    }

    /// Live spend of the key in its current period.
    pub fn spend_nanos(&self) -> i64 {
        self.key.spend.load(Ordering::Relaxed)
    }

    /// Blocked or expired: the key may not be used at all.
    pub fn usable(&self, now: DateTime<Utc>) -> AppResult<()> {
        if self.blocked {
            return Err(AppError::Unauthorized("this API key is blocked".into()));
        }
        if self.expires_at.is_some_and(|at| at <= now) {
            return Err(AppError::Unauthorized("this API key has expired".into()));
        }
        Ok(())
    }

    /// Blocked, expiry, budgets and rate limits; then takes a place among
    /// the requests in flight and counts the request. Spend and tokens are
    /// added when it finishes ([`Self::charge`]), so concurrent requests can
    /// overshoot a budget by what they cost.
    // ponytail: check-then-charge; reserve an estimate up front if hard caps matter.
    pub fn admit(&self, now: DateTime<Utc>) -> AppResult<Slot> {
        self.usable(now)?;
        let minute = minute(now);
        for limits in self.limits() {
            limits.check(minute)?;
        }
        let mut slot = Slot::default();
        for limits in self.limits() {
            let running = limits.window.in_flight.fetch_add(1, Ordering::Relaxed) + 1;
            slot.0.push(limits.window.clone());
            if limits.parallel.is_some_and(|max| running > max) {
                // Dropping `slot` gives back every place taken so far.
                return Err(AppError::RateLimited(format!(
                    "this {} has reached its limit of parallel requests",
                    limits.who
                )));
            }
        }
        for limits in self.limits() {
            limits.window.add(minute, 1, 0);
        }
        Ok(slot)
    }

    /// Adds a finished request's cost and tokens to the live figures.
    pub(crate) fn charge(&self, cost_nanos: i64, tokens: i64) {
        let minute = minute(Utc::now());
        for limits in self.limits() {
            limits.spend.fetch_add(cost_nanos, Ordering::Relaxed);
            limits.window.add(minute, 0, tokens);
        }
    }
}

pub struct Gateway {
    pool: DbPool,
    secrets: SecretBox,
    pub(crate) http: reqwest::Client,
    usage: mpsc::Sender<Message>,
    /// Bumped by every invalidation, so a load that raced one is not cached.
    generation: AtomicU64,
    keys: RwLock<HashMap<String, Arc<KeyContext>>>,
    routes: RwLock<Option<Routes>>,
    key_spend: Mutex<HashMap<i64, Arc<AtomicI64>>>,
    team_spend: Mutex<HashMap<i64, Arc<AtomicI64>>>,
    key_windows: Mutex<HashMap<i64, Arc<Window>>>,
    team_windows: Mutex<HashMap<i64, Arc<Window>>>,
    health: router::HealthMap,
    catalog: RwLock<Arc<Catalog>>,
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: i64,
    team_id: Option<i64>,
    user_id: Option<i64>,
    log_bodies: bool,
    blocked: bool,
    expires_at: Option<DateTime<Utc>>,
    max_budget_nanos: Option<i64>,
    spend_nanos: i64,
    budget_duration: Option<String>,
    budget_reset_at: Option<DateTime<Utc>>,
    rpm_limit: Option<i64>,
    tpm_limit: Option<i64>,
    max_parallel_requests: Option<i64>,
    team_budget: Option<i64>,
    team_spend: Option<i64>,
    team_duration: Option<String>,
    team_reset_at: Option<DateTime<Utc>>,
    team_rpm: Option<i64>,
    team_tpm: Option<i64>,
    team_parallel: Option<i64>,
    team_all_models: Option<bool>,
}

impl Gateway {
    /// Starts the usage writer, so this must run inside a Tokio runtime.
    pub fn new(pool: DbPool, secrets: SecretBox, upstream_timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Per read, not per request: a long stream is fine while it moves.
            .read_timeout(upstream_timeout)
            .build()
            .expect("the HTTP client must build");
        Self {
            usage: usage::spawn_writer(pool.clone()),
            pool,
            secrets,
            http,
            generation: AtomicU64::new(0),
            keys: RwLock::default(),
            routes: RwLock::default(),
            key_spend: Mutex::default(),
            team_spend: Mutex::default(),
            key_windows: Mutex::default(),
            team_windows: Mutex::default(),
            health: Mutex::default(),
            catalog: RwLock::new(Catalog::builtin()),
        }
    }

    /// The price catalog models are priced by.
    pub fn catalog(&self) -> Arc<Catalog> {
        self.catalog.read().expect("poisoned").clone()
    }

    /// Replaces the catalog (after a sync) and reprices every route.
    pub fn set_catalog(&self, catalog: Arc<Catalog>) {
        *self.catalog.write().expect("poisoned") = catalog;
        self.invalidate();
    }

    /// Drops every cached key and model, so the next request reads the
    /// database. Called after any change to keys, teams or models.
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.keys.write().expect("poisoned").clear();
        *self.routes.write().expect("poisoned") = None;
    }

    /// Waits until every usage event recorded so far is in the database.
    pub async fn flush(&self) {
        let (done, wait) = tokio::sync::oneshot::channel();
        if self.usage.send(Message::Flush(done)).await.is_ok() {
            let _ = wait.await;
        }
    }

    pub(crate) fn usage_sender(&self) -> mpsc::Sender<Message> {
        self.usage.clone()
    }

    /// Resolves a raw virtual key and admits a request for it: a budget
    /// whose period ended is reset first (lazily: no timer runs).
    pub async fn admit(&self, raw_key: &str) -> AppResult<(Arc<KeyContext>, Slot)> {
        self.admit_hash(&hash_key(raw_key)).await
    }

    /// [`Self::admit`] for a key known by its hash: the dashboard's
    /// Playground, which never holds the raw key.
    pub async fn admit_hash(&self, hash: &str) -> AppResult<(Arc<KeyContext>, Slot)> {
        let now = Utc::now();
        let mut key = self.authenticate_hash(hash).await?;
        if key.limits().any(|limits| limits.due(now)) {
            self.reset_budgets(&key, now).await?;
            key = self.authenticate_hash(hash).await?;
        }
        let slot = key.admit(now)?;
        Ok((key, slot))
    }

    /// Starts a new period for every budget of `key` that is due. The guard
    /// on `budget_reset_at` makes concurrent resets happen once.
    // ponytail: usage still in the writer's queue lands in the new period.
    async fn reset_budgets(&self, key: &KeyContext, now: DateTime<Utc>) -> AppResult<()> {
        for limits in key.limits().filter(|limits| limits.due(now)) {
            let table = if limits.who == "key" {
                "api_keys"
            } else {
                "teams"
            };
            let next = limits.period.as_deref().and_then(|p| next_reset(p, now));
            // `table` is one of the two literals above.
            let reset = sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {table} SET spend_nanos = 0, budget_reset_at = $1
                 WHERE id = $2 AND budget_reset_at <= $3"
            )))
            .bind(next)
            .bind(limits.id)
            .bind(now)
            .execute(&self.pool)
            .await?
            .rows_affected();
            if reset > 0 {
                limits.spend.store(0, Ordering::Relaxed);
            }
        }
        self.invalidate();
        Ok(())
    }

    /// Resolves a raw virtual key. Revoked and unknown keys look the same.
    pub async fn authenticate(&self, raw_key: &str) -> AppResult<Arc<KeyContext>> {
        self.authenticate_hash(&hash_key(raw_key)).await
    }

    pub async fn authenticate_hash(&self, hash: &str) -> AppResult<Arc<KeyContext>> {
        if let Some(context) = self.keys.read().expect("poisoned").get(hash) {
            return Ok(context.clone());
        }
        let generation = self.generation.load(Ordering::SeqCst);
        let row = sqlx::query_as::<_, KeyRow>(
            "SELECT k.id, k.team_id, k.user_id, k.log_bodies, k.blocked, k.expires_at, k.max_budget_nanos,
                    k.spend_nanos, k.budget_duration, k.budget_reset_at, k.rpm_limit, k.tpm_limit,
                    k.max_parallel_requests, t.max_parallel_requests AS team_parallel,
                    t.max_budget_nanos AS team_budget, t.spend_nanos AS team_spend,
                    t.budget_duration AS team_duration, t.budget_reset_at AS team_reset_at,
                    t.rpm_limit AS team_rpm, t.tpm_limit AS team_tpm,
                    t.all_models AS team_all_models
             FROM api_keys k LEFT JOIN teams t ON t.id = k.team_id
             WHERE k.key_hash = $1 AND k.revoked_at IS NULL",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid API key".into()))?;

        let key_models = self.model_names("key_models", "key_id", row.id).await?;
        // `None`: every model (a personal key, or a team with all models).
        let team_models = match row.team_id {
            Some(team_id) if row.team_all_models != Some(true) => {
                Some(self.model_names("team_models", "team_id", team_id).await?)
            }
            _ => None,
        };
        let allowed = match (key_models.is_empty(), team_models) {
            (true, team) => team,
            (false, None) => Some(key_models),
            (false, Some(team)) => Some(key_models.intersection(&team).cloned().collect()),
        };

        let team = row.team_id.map(|id| Limits {
            who: "team",
            id,
            budget: row.team_budget,
            period: row.team_duration.clone(),
            reset_at: row.team_reset_at,
            rpm: row.team_rpm,
            tpm: row.team_tpm,
            parallel: row.team_parallel,
            spend: shared(&self.team_spend, id, || {
                AtomicI64::new(row.team_spend.unwrap_or(0))
            }),
            window: shared(&self.team_windows, id, Window::default),
        });
        let context = Arc::new(KeyContext {
            key_id: row.id,
            team_id: row.team_id,
            user_id: row.user_id,
            log_bodies: row.log_bodies,
            blocked: row.blocked,
            expires_at: row.expires_at,
            allowed,
            key: Limits {
                who: "key",
                id: row.id,
                budget: row.max_budget_nanos,
                period: row.budget_duration,
                reset_at: row.budget_reset_at,
                rpm: row.rpm_limit,
                tpm: row.tpm_limit,
                parallel: row.max_parallel_requests,
                spend: shared(&self.key_spend, row.id, || AtomicI64::new(row.spend_nanos)),
                window: shared(&self.key_windows, row.id, Window::default),
            },
            team,
        });
        if self.generation.load(Ordering::SeqCst) == generation {
            self.keys
                .write()
                .expect("poisoned")
                .insert(hash.to_string(), context.clone());
        }
        Ok(context)
    }

    async fn model_names(&self, table: &str, column: &str, id: i64) -> AppResult<HashSet<String>> {
        // Both identifiers are compile-time constants from `authenticate`.
        let sql = format!(
            "SELECT m.name FROM {table} x JOIN models m ON m.id = x.model_id WHERE x.{column} = $1"
        );
        Ok(sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect())
    }

    /// Every active deployment by name, and the router settings.
    pub async fn routes(&self) -> AppResult<Routes> {
        if let Some(routes) = self.routes.read().expect("poisoned").as_ref() {
            return Ok(routes.clone());
        }
        let generation = self.generation.load(Ordering::SeqCst);
        let models = crate::services::models::list(&self.pool).await?;
        let settings = crate::services::settings::router(&self.pool).await?;
        let catalog = self.catalog();
        let mut groups: HashMap<String, Vec<Arc<Route>>> = HashMap::new();
        for model in models.into_iter().filter(|m| m.is_active) {
            let (Some(provider), Some(api_base)) =
                (providers::find(&model.provider), model.effective_api_base())
            else {
                log::error!("model '{}' has no usable provider; skipping it", model.name);
                continue;
            };
            let api_key = match &model.api_key_encrypted {
                Some(sealed) => match self.secrets.decrypt(sealed) {
                    Ok(key) => Some(key),
                    Err(e) => {
                        log::error!("model '{}' skipped: {e}", model.name);
                        continue;
                    }
                },
                None => None,
            };
            let (pricing, _) = crate::services::models::effective_pricing(&model, &catalog);
            let listed = model
                .catalog_key
                .as_deref()
                .and_then(|key| catalog.get(key));
            groups
                .entry(model.name.clone())
                .or_default()
                .push(Arc::new(Route {
                    id: model.id,
                    name: model.name,
                    provider: model.provider,
                    wire: provider.wire,
                    stream_usage: provider.stream_usage,
                    native_responses: provider.native_responses,
                    upstream_model: model.upstream_model,
                    api_base,
                    api_version: model.api_version,
                    api_key,
                    pricing: pricing.unwrap_or_default(),
                    max_output_tokens: listed.and_then(|e| e.max_output_tokens),
                    supports_native_structured_output: listed
                        .is_some_and(|e| e.supports_native_structured_output),
                    created_at: model.created_at,
                    weight: model.weight,
                }));
        }
        let routes = Arc::new(RouteTable { groups, settings });
        if self.generation.load(Ordering::SeqCst) == generation {
            *self.routes.write().expect("poisoned") = Some(routes.clone());
        }
        Ok(routes)
    }

    /// Checks that `model` exists and `key` may call it.
    pub async fn check_model(&self, key: &KeyContext, model: &str) -> AppResult<()> {
        if !self.routes().await?.groups.contains_key(model) {
            return Err(AppError::NotFound(format!(
                "model '{model}' does not exist"
            )));
        }
        if !key.may_call(model) {
            return Err(AppError::Forbidden(format!(
                "this key is not allowed to call model '{model}'"
            )));
        }
        Ok(())
    }

    /// One deployment of `model`, without retries: for calls that are not
    /// worth routing (token counting).
    pub async fn any_route(&self, key: &KeyContext, model: &str) -> AppResult<Arc<Route>> {
        self.check_model(key, model).await?;
        let routes = self.routes().await?;
        Ok(routes.groups[model][0].clone())
    }

    /// The model names `key` may call, each with its first deployment.
    pub async fn allowed_routes(&self, key: &KeyContext) -> AppResult<Vec<Arc<Route>>> {
        let mut routes: Vec<_> = self
            .routes()
            .await?
            .groups
            .iter()
            .filter(|(name, _)| key.may_call(name))
            .map(|(_, deployments)| deployments[0].clone())
            .collect();
        routes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(routes)
    }
}

/// The live figure for `id`, made by `init` (from the database) the first time.
fn shared<T>(map: &Mutex<HashMap<i64, Arc<T>>>, id: i64, init: impl FnOnce() -> T) -> Arc<T> {
    map.lock()
        .expect("poisoned")
        .entry(id)
        .or_insert_with(|| Arc::new(init()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(budget: Option<i64>, spent: i64, rpm: Option<i64>) -> Limits {
        Limits {
            who: "key",
            id: 1,
            budget,
            period: None,
            reset_at: None,
            rpm,
            tpm: None,
            parallel: None,
            spend: Arc::new(AtomicI64::new(spent)),
            window: Arc::default(),
        }
    }

    fn context(allowed: Option<&[&str]>, key_budget: Option<i64>, spent: i64) -> KeyContext {
        KeyContext {
            key_id: 1,
            team_id: None,
            user_id: Some(1),
            log_bodies: false,
            blocked: false,
            expires_at: None,
            allowed: allowed.map(|names| names.iter().map(|n| n.to_string()).collect()),
            key: limits(key_budget, spent, None),
            team: None,
        }
    }

    #[test]
    fn requests_positive_limit_start_over_each_minute() {
        let mut key = context(None, None, 0);
        key.key = limits(None, 0, Some(1));
        let now = Utc::now();
        assert!(key.admit(now).is_ok());
        assert!(matches!(key.admit(now), Err(AppError::RateLimited(_))));
        assert!(key.admit(now + chrono::Duration::seconds(60)).is_ok());
    }

    #[test]
    fn no_allowlist_means_every_model() {
        assert!(context(None, None, 0).may_call("anything"));
        let limited = context(Some(&["gpt-4o"]), None, 0);
        assert!(limited.may_call("gpt-4o"));
        assert!(!limited.may_call("o3"));
    }

    #[test]
    fn a_budget_is_spent_once_spend_reaches_it() {
        assert!(context(None, Some(100), 99).admit(Utc::now()).is_ok());
        assert!(matches!(
            context(None, Some(100), 100).admit(Utc::now()),
            Err(AppError::BudgetExceeded(_))
        ));
    }

    #[test]
    fn openai_usage_reads_both_counts() {
        let usage =
            Usage::from_openai(&json!({"prompt_tokens": 3, "completion_tokens": 4})).unwrap();
        assert_eq!(usage.to_json()["total_tokens"], 7);
        assert!(Usage::from_openai(&Value::Null).is_none());
    }
}
