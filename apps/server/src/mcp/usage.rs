use actix_web::body::MessageBody;
use actix_web::{web, HttpResponse};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{args, json, reply, LlmtrackMcp};
use crate::error::{AppError, AppResult};
use crate::models::list::ListQuery;
use crate::routes;
use crate::services::bodies;
use crate::services::usage::{self, LogQuery};

/// Characters of conversation shown on each side of the first match.
const SNIPPET_RADIUS: usize = 120;
/// Hits per kind of thing in `search`.
const SEARCH_HITS: i64 = 5;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct UsageArgs {
    /// First day, `YYYY-MM-DD` (default: 30 days ago)
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    /// Last day, `YYYY-MM-DD` (default: today)
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(flatten)]
    scope: ScopeArgs,
    /// What the daily series splits by: `model` (default), `team`, `key`, `person`, `end_user` or `tag`
    #[serde(skip_serializing_if = "Option::is_none")]
    group_by: Option<String>,
}

/// Each list keeps only its values (every value when empty); the `exclude_`
/// lists then leave values out. E.g. a team without one key:
/// `team_id: [4], exclude_key_id: [17]`.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ScopeArgs {
    /// Team IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    team_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_team_id: Vec<i64>,
    /// Key IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    key_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_key_id: Vec<i64>,
    /// Public model names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    model: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_model: Vec<String>,
    /// Person IDs (the keys' spend while assigned to them)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    person_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_person_id: Vec<i64>,
    /// Label IDs (keys carrying one of them now)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    label_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_label_id: Vec<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RequestsArgs {
    /// Page number (1-indexed, default 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i64>,
    /// Items per page (default 20, max 200)
    #[serde(skip_serializing_if = "Option::is_none")]
    per_page: Option<i64>,
    /// `created_at`, `cost`, `tokens` or `latency`, `-` first for descending (default `-created_at`)
    #[serde(skip_serializing_if = "Option::is_none")]
    sort: Option<String>,
    /// Words said in the conversation (request or reply), all of them, any order or case.
    /// Uses the full-text index; only keys that keep bodies are searchable.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Substring of the model name, customer or request id
    #[serde(skip_serializing_if = "Option::is_none")]
    q: Option<String>,
    /// Team IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    team_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_team_id: Vec<i64>,
    /// Key IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    key_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_key_id: Vec<i64>,
    /// Exact public model names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    model: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_model: Vec<String>,
    /// Label IDs (keys carrying one of them now)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    label_id: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_label_id: Vec<i64>,
    /// `success` or `error`
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    /// From this time, RFC 3339 (e.g. `2026-10-01T00:00:00Z`)
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    /// Up to this time, RFC 3339
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    /// Customers (end users) the requests were made for
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    end_user: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_end_user: Vec<String>,
    /// Request tags; a request matches by any of its tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tag: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_tag: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RequestArgs {
    /// The request's `request_id`
    request_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SearchArgs {
    /// Names to look for, or words said in a conversation
    query: String,
}

#[tool_router(router = usage_tools, vis = "pub(super)")]
impl LlmtrackMcp {
    #[tool(
        description = "Spend and traffic over a date range, for any combination of teams, keys, models, people and labels kept or left out: totals, the previous period, a daily series and breakdowns by team, key, person, model, customer and tag. Read from daily rollups, so cheap for any range.",
        annotations(read_only_hint = true)
    )]
    async fn get_usage(&self, Parameters(a): Parameters<UsageArgs>) -> CallToolResult {
        reply(async {
            routes::usage::report(self.state(), self.current(), web::Query(args(&a)?)).await
        })
        .await
    }

    #[tool(
        description = "Filter the request log: who called which model, status, tokens, cost, latency, customer and tags. Every filter takes several values and has an `exclude_` twin (e.g. a team without one key). With `text`, only requests whose conversation contains those words, each with a snippet around the match. Combine filters to narrow down; follow up with get_request.",
        annotations(read_only_hint = true)
    )]
    async fn list_requests(&self, Parameters(a): Parameters<RequestsArgs>) -> CallToolResult {
        json(async {
            let mut list: ListQuery = args(&a)?;
            list.per_page = list.per_page.or(Some(20));
            self.requests(&list, &args(&a)?).await
        })
        .await
    }

    #[tool(
        description = "One request in full: its log line and, if its key keeps bodies, the conversation as sent (`body.request`) and the reply (`body.response`).",
        annotations(read_only_hint = true)
    )]
    async fn get_request(&self, Parameters(a): Parameters<RequestArgs>) -> CallToolResult {
        json(async {
            let list = ListQuery {
                q: Some(a.request_id.clone()),
                per_page: Some(1),
                ..Default::default()
            };
            let query: LogQuery = args(&json!({}))?;
            let page = usage::logs(&self.state.pool, &self.user, &list, &query).await?;
            let log = page
                .data
                .into_iter()
                .find(|l| l.entry.request_id == a.request_id)
                .ok_or_else(|| AppError::NotFound(format!("request {}", a.request_id)))?;
            let body = match bodies::get(&self.state.pool, &self.user, &a.request_id).await {
                Ok(body) => Some(body),
                Err(AppError::NotFound(_)) => None,
                Err(e) => return Err(e),
            };
            Ok(json!({ "log": log, "body": body }))
        })
        .await
    }

    #[tool(
        description = "Find anything in one call: teams, keys, people, models, labels and users whose name contains the query, and the requests whose conversation contains its words (newest first, with snippets). Each list shows its `total`; narrow down with the list_* tools.",
        annotations(read_only_hint = true)
    )]
    async fn search(&self, Parameters(a): Parameters<SearchArgs>) -> CallToolResult {
        json(async {
            let names = json!({ "q": a.query, "per_page": SEARCH_HITS });
            let q = || args(&names).map(web::Query);
            let (state, me) = (self.state(), || self.current());
            let mut found = json!({
                "teams": hits(routes::teams::list(state.clone(), me(), q()?, none()?).await),
                "keys": hits(routes::keys::list(state.clone(), me(), q()?, none()?).await),
                "people": hits(routes::people::list(state.clone(), me(), q()?, none()?).await),
                "models": hits(routes::models::list(state.clone(), me(), q()?, none()?).await),
                "labels": hits(routes::labels::list(state.clone(), me(), q()?).await),
            });
            if let Ok(admin) = self.admin() {
                found["users"] = hits(routes::users::list(state, admin, q()?, none()?).await);
            }
            let requests = ListQuery {
                per_page: Some(SEARCH_HITS * 2),
                ..Default::default()
            };
            let query: LogQuery = args(&json!({ "text": a.query }))?;
            found["requests"] = self.requests(&requests, &query).await?;
            Ok(found)
        })
        .await
    }
}

impl LlmtrackMcp {
    /// A page of the request log as filtered, each line with a snippet of its
    /// conversation when the filter searched one.
    async fn requests(&self, list: &ListQuery, query: &LogQuery) -> AppResult<Value> {
        let page = usage::logs(&self.state.pool, &self.user, list, query).await?;
        let words: Vec<String> = query
            .text
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut snippets = Vec::with_capacity(page.data.len());
        for line in &page.data {
            let mut snippet_of = None;
            if line.entry.has_body && !words.is_empty() {
                snippet_of = bodies::get(&self.state.pool, &self.user, &line.entry.request_id)
                    .await
                    .ok()
                    .map(|body| snippet(&body, &words));
            }
            let snippet = snippet_of;
            snippets.push(snippet);
        }
        let mut page = serde_json::to_value(page).map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(lines) = page["data"].as_array_mut() {
            for (line, snippet) in lines.iter_mut().zip(snippets) {
                if let Some(snippet) = snippet {
                    line["snippet"] = Value::String(snippet);
                }
            }
        }
        Ok(page)
    }
}

/// A list's own filters, none set.
fn none<T: serde::de::DeserializeOwned>() -> AppResult<web::Query<T>> {
    args(&json!({})).map(web::Query)
}

/// What a list handler answered, or the error it gave, as one section of
/// `search`: a section the caller may not read says why instead.
fn hits(response: AppResult<HttpResponse>) -> Value {
    let body = response.map(|r| r.into_body().try_into_bytes().unwrap_or_default());
    match body {
        Ok(bytes) => {
            let mut page: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            if let Some(map) = page.as_object_mut() {
                map.remove("page");
                map.remove("per_page");
            }
            page
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// The conversation's text around the first of `words` it contains: every
/// string of the request and the reply, so roles and messages read as text
/// rather than JSON. The start of it when no word is found as typed (the
/// index also matches without accents).
fn snippet(body: &bodies::Body, words: &[String]) -> String {
    let mut text = String::new();
    strings(&body.request, &mut text);
    if let Some(response) = &body.response {
        strings(response, &mut text);
    }
    let lower = text.to_lowercase();
    // Lowercasing can change byte lengths (rarely); then search and show the
    // lowercase text, so offsets stay valid.
    let shown = if lower.len() == text.len() {
        &text
    } else {
        &lower
    };
    let at = words
        .iter()
        .filter_map(|w| lower.find(w.as_str()))
        .min()
        .unwrap_or(0);
    let start = shown[..at]
        .char_indices()
        .rev()
        .nth(SNIPPET_RADIUS - 1)
        .map_or(0, |(i, _)| i);
    let end = shown[at..]
        .char_indices()
        .nth(SNIPPET_RADIUS * 2)
        .map_or(shown.len(), |(i, _)| at + i);
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(shown[start..end].trim());
    if end < shown.len() {
        out.push('…');
    }
    out
}

/// Every string value in `value`, one per line.
fn strings(value: &Value, out: &mut String) {
    match value {
        Value::String(s) => {
            out.push_str(s);
            out.push('\n');
        }
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| strings(v, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_is_the_text_around_the_first_match() {
        let filler = "x ".repeat(200);
        let body = bodies::Body {
            request: json!({"model": "gpt", "messages": [{"role": "user", "content": format!("{filler}Where is my REFUND? {filler}")}]}),
            response: Some(json!({"choices": [{"message": {"content": "It was sent."}}]})),
        };
        let s = snippet(&body, &["refund".into()]);
        assert!(s.starts_with('…') && s.ends_with('…'), "{s}");
        assert!(s.contains("Where is my REFUND?"), "{s}");
        assert!(s.chars().count() <= SNIPPET_RADIUS * 3 + 2, "{s}");

        let s = snippet(&body, &["sent".into()]);
        assert!(s.ends_with("It was sent."), "{s}");
        // Not found as typed: the start of the conversation.
        assert!(!snippet(&body, &["nowhere".into()]).starts_with('…'));
    }
}
