//! The router: a model name is a group of deployments. A request goes
//! to a healthy one picked by weight, is retried on another when it fails,
//! and falls back to other model names when the whole group cannot answer.
//!
//! Defaults: two retries, a deployment cools down for five
//! seconds after a 429/401/402/403/404/408 or more than three failures in a
//! minute, and a group with a single deployment never cools down.

use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::HttpResponse;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{Gateway, KeyContext, Route};
use crate::error::{AppError, AppResult};

/// `[{"gpt-4o": ["claude-sonnet", "gpt-4o-mini"]}, {"*": ["gpt-4o-mini"]}]`:
/// One model name (or `*` for any) per entry.
pub type FallbackList = Vec<BTreeMap<String, Vec<String>>>;

/// The router's settings (`router_settings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RouterSettings {
    pub num_retries: i64,
    /// Failures a deployment may have in a minute before it cools down.
    pub allowed_fails: i64,
    /// Seconds a deployment sits out.
    pub cooldown_time: i64,
    pub fallbacks: FallbackList,
    pub context_window_fallbacks: FallbackList,
    pub content_policy_fallbacks: FallbackList,
}

impl Default for RouterSettings {
    fn default() -> Self {
        Self {
            num_retries: 2,
            allowed_fails: 3,
            cooldown_time: 5,
            fallbacks: Vec::new(),
            context_window_fallbacks: Vec::new(),
            content_policy_fallbacks: Vec::new(),
        }
    }
}

/// The model names `list` falls back to from `model`: its own entry, else
/// the `*` entry.
fn fallbacks_for<'a>(list: &'a FallbackList, model: &str) -> Option<&'a Vec<String>> {
    let entry = |name: &str| list.iter().find_map(|entry| entry.get(name));
    entry(model).or_else(|| entry("*"))
}

/// What a failed attempt means for retrying, cooling down and falling back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// 408, 409, 429, 5xx, unreachable: worth another try.
    Retryable {
        cool_now: bool,
    },
    /// The provider refused the gateway's credentials (401/402/403): another
    /// deployment may hold good ones.
    Credentials,
    /// 404: this deployment does not serve the model.
    NotFound,
    ContextWindow,
    ContentPolicy,
    /// Nothing healthy to try.
    NoDeployments,
    /// The request itself is wrong; no other deployment would do better.
    Final,
}

impl Failure {
    pub fn of(error: &AppError) -> Self {
        let AppError::Provider {
            status, message, ..
        } = error.kind()
        else {
            return match error.kind() {
                AppError::Upstream(_) => Failure::Retryable { cool_now: false },
                _ => Failure::Final,
            };
        };
        match status {
            None => Failure::Retryable { cool_now: false },
            Some(408 | 429) => Failure::Retryable { cool_now: true },
            Some(401..=403) => Failure::Credentials,
            Some(404) => Failure::NotFound,
            Some(409) => Failure::Retryable { cool_now: false },
            Some(400) if is_context_window(message) => Failure::ContextWindow,
            Some(400) if is_content_policy(message) => Failure::ContentPolicy,
            Some(s) if *s >= 500 => Failure::Retryable { cool_now: false },
            Some(_) => Failure::Final,
        }
    }

    fn cools_now(self) -> bool {
        matches!(
            self,
            Failure::Retryable { cool_now: true } | Failure::Credentials | Failure::NotFound
        )
    }

    fn counts(self) -> bool {
        matches!(self, Failure::Retryable { .. }) || self.cools_now()
    }
}

/// The phrases providers word "the prompt does not fit" with.
fn is_context_window(message: &str) -> bool {
    let m = message.to_lowercase();
    [
        "context_length_exceeded",
        "context length",
        "context window",
        "prompt is too long",
        "input is too long",
        "too many tokens",
        "reduce the length",
    ]
    .iter()
    .any(|p| m.contains(p))
}

fn is_content_policy(message: &str) -> bool {
    let m = message.to_lowercase();
    [
        "content_policy_violation",
        "content policy",
        "content management policy",
        "content_filter",
        "safety system",
    ]
    .iter()
    .any(|p| m.contains(p))
}

/// Recent failures of one deployment and when its cooldown ends.
#[derive(Debug, Default)]
pub(crate) struct Health {
    fails: VecDeque<Instant>,
    until: Option<Instant>,
}

/// Per-deployment health, kept across cache invalidations.
// ponytail: per process; several replicas need a shared store (Redis).
pub(crate) type HealthMap = Mutex<HashMap<i64, Health>>;

impl Gateway {
    fn cooling(&self, id: i64, now: Instant) -> bool {
        self.health
            .lock()
            .expect("poisoned")
            .get(&id)
            .and_then(|h| h.until)
            .is_some_and(|until| until > now)
    }

    fn failed(&self, id: i64, failure: Failure, alone: bool, settings: &RouterSettings) {
        if !failure.counts() || alone {
            return;
        }
        let now = Instant::now();
        let mut map = self.health.lock().expect("poisoned");
        let health = map.entry(id).or_default();
        health.fails.push_back(now);
        while health
            .fails
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            health.fails.pop_front();
        }
        let too_many = health.fails.len() as i64 > settings.allowed_fails;
        if failure.cools_now() || too_many {
            let seconds = u64::try_from(settings.cooldown_time).unwrap_or(0);
            health.until = Some(now + Duration::from_secs(seconds));
        }
    }
}

/// Picks a deployment by simple shuffle: by weight when any
/// has one, evenly otherwise.
fn pick(candidates: &[Arc<Route>]) -> Arc<Route> {
    let total: i64 = candidates.iter().filter_map(|r| r.weight).sum();
    let mut rng = rand::rng();
    if total > 0 {
        let mut ticket = rng.random_range(0..total);
        for route in candidates {
            let weight = route.weight.unwrap_or(0);
            if ticket < weight {
                return route.clone();
            }
            ticket -= weight;
        }
    }
    candidates[rng.random_range(0..candidates.len())].clone()
}

/// Wait before trying the same deployment again: 0.5 s doubling up to 8 s.
fn backoff(attempt: u32) -> Duration {
    Duration::from_millis((500u64 << attempt.saturating_sub(1).min(4)).min(8000))
}

fn no_deployments(model: &str, settings: &RouterSettings) -> AppError {
    AppError::RateLimited(format!(
        "No deployments available for selected model, try again in {} seconds. Passed model={model}",
        settings.cooldown_time
    ))
}

/// Sends a request for `model` through the router. `attempt` makes one call
/// to one deployment; it is called again for every retry and fallback.
pub async fn run<F, Fut>(
    gateway: &Gateway,
    key: &KeyContext,
    model: &str,
    mut attempt: F,
) -> AppResult<HttpResponse>
where
    F: FnMut(Arc<Route>) -> Fut,
    Fut: Future<Output = AppResult<HttpResponse>>,
{
    let table = gateway.routes().await?;
    let settings = &table.settings;
    let mut retries = 0;
    let (error, failure) = match try_group(
        gateway,
        &table.groups,
        model,
        settings,
        &mut attempt,
        &mut retries,
    )
    .await
    {
        Ok((response, route)) => return Ok(tagged(response, &route, retries, 0)),
        Err(failed) => failed,
    };
    let chain = match failure {
        Failure::Final => None,
        Failure::ContextWindow => fallbacks_for(&settings.context_window_fallbacks, model)
            .or_else(|| fallbacks_for(&settings.fallbacks, model)),
        Failure::ContentPolicy => fallbacks_for(&settings.content_policy_fallbacks, model)
            .or_else(|| fallbacks_for(&settings.fallbacks, model)),
        _ => fallbacks_for(&settings.fallbacks, model),
    };
    let mut fallbacks = 0;
    for group in chain.into_iter().flatten() {
        if group == model || !key.may_call(group) || !table.groups.contains_key(group) {
            continue;
        }
        fallbacks += 1;
        if let Ok((response, route)) = try_group(
            gateway,
            &table.groups,
            group,
            settings,
            &mut attempt,
            &mut retries,
        )
        .await
        {
            return Ok(tagged(response, &route, retries, fallbacks));
        }
    }
    // The original model's error says most about what went wrong.
    Err(error)
}

/// One model name, with its retries.
async fn try_group<F, Fut>(
    gateway: &Gateway,
    groups: &HashMap<String, Vec<Arc<Route>>>,
    model: &str,
    settings: &RouterSettings,
    attempt: &mut F,
    retries: &mut u32,
) -> Result<(HttpResponse, Arc<Route>), (AppError, Failure)>
where
    F: FnMut(Arc<Route>) -> Fut,
    Fut: Future<Output = AppResult<HttpResponse>>,
{
    let deployments = groups.get(model).map(Vec::as_slice).unwrap_or_default();
    let alone = deployments.len() <= 1;
    let mut tried = HashSet::new();
    let mut last = None;
    for n in 0..=u32::try_from(settings.num_retries).unwrap_or(0) {
        let now = Instant::now();
        // A lone deployment never cools down: skip the health lock.
        let healthy: Vec<Arc<Route>> = deployments
            .iter()
            .filter(|r| alone || !gateway.cooling(r.id, now))
            .cloned()
            .collect();
        if healthy.is_empty() {
            break;
        }
        let fresh: Vec<Arc<Route>> = healthy
            .iter()
            .filter(|r| !tried.contains(&r.id))
            .cloned()
            .collect();
        let route = pick(if fresh.is_empty() { &healthy } else { &fresh });
        if n > 0 {
            *retries += 1;
            if fresh.is_empty() {
                actix_web::rt::time::sleep(backoff(n)).await;
            }
        }
        match attempt(route.clone()).await {
            Ok(response) => return Ok((response, route)),
            Err(error) => {
                let failure = Failure::of(&error);
                gateway.failed(route.id, failure, alone, settings);
                tried.insert(route.id);
                let again = match failure {
                    Failure::Retryable { .. } => true,
                    Failure::Credentials => !alone,
                    _ => false,
                };
                last = Some((error, failure));
                if !again {
                    break;
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| (no_deployments(model, settings), Failure::NoDeployments)))
}

/// Response headers: which deployment answered and how it got there.
fn tagged(mut response: HttpResponse, route: &Route, retries: u32, fallbacks: u32) -> HttpResponse {
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-litellm-model-id", route.id.to_string()),
        ("x-litellm-model-group", route.name.clone()),
        ("x-litellm-attempted-retries", retries.to_string()),
        ("x-litellm-attempted-fallbacks", fallbacks.to_string()),
    ] {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(HeaderName::from_static(name), value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(status: u16, message: &str) -> AppError {
        AppError::Provider {
            status: Some(status),
            message: message.into(),
            body: None,
        }
    }

    #[test]
    fn failures_are_classified() {
        assert_eq!(
            Failure::of(&provider(429, "slow down")),
            Failure::Retryable { cool_now: true }
        );
        assert_eq!(
            Failure::of(&provider(503, "busy")),
            Failure::Retryable { cool_now: false }
        );
        assert_eq!(Failure::of(&provider(401, "bad key")), Failure::Credentials);
        assert_eq!(Failure::of(&provider(404, "no model")), Failure::NotFound);
        assert_eq!(
            Failure::of(&provider(
                400,
                "This model's maximum context length is 8192"
            )),
            Failure::ContextWindow
        );
        assert_eq!(
            Failure::of(&provider(400, "flagged by our content_filter")),
            Failure::ContentPolicy
        );
        assert_eq!(Failure::of(&provider(400, "bad field")), Failure::Final);
        assert_eq!(
            Failure::of(&AppError::Validation("x".into())),
            Failure::Final
        );
        let unreachable = AppError::Provider {
            status: None,
            message: "refused".into(),
            body: None,
        };
        assert_eq!(
            Failure::of(&unreachable),
            Failure::Retryable { cool_now: false }
        );
    }

    #[test]
    fn a_model_falls_back_by_name_then_by_star() {
        let list: FallbackList =
            serde_json::from_str(r#"[{"gpt-4o": ["a", "b"]}, {"*": ["c"]}]"#).unwrap();
        assert_eq!(fallbacks_for(&list, "gpt-4o").unwrap(), &["a", "b"]);
        assert_eq!(fallbacks_for(&list, "other").unwrap(), &["c"]);
        assert!(fallbacks_for(&Vec::new(), "gpt-4o").is_none());
    }

    #[test]
    fn backoff_doubles_up_to_eight_seconds() {
        assert_eq!(backoff(1), Duration::from_millis(500));
        assert_eq!(backoff(2), Duration::from_millis(1000));
        assert_eq!(backoff(9), Duration::from_millis(8000));
    }

    #[test]
    fn settings_have_defaults_and_reject_unknown_fields() {
        let partial: RouterSettings = serde_json::from_str(r#"{"num_retries": 0}"#).unwrap();
        assert_eq!(partial.num_retries, 0);
        assert_eq!(partial.cooldown_time, 5);
        assert!(serde_json::from_str::<RouterSettings>(r#"{"retries": 1}"#).is_err());
    }
}
