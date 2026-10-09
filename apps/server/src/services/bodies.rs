//! Request and response bodies, for the keys that keep them: read through
//! their log line, purged once their key's retention passes, exported as
//! JSON Lines.

use actix_web::web::Bytes;
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::list::ListQuery;
use crate::models::money::nanos_to_usd;
use crate::models::user::User;
use crate::services::usage::{bind_log_filter, LogFilter, LogQuery, LOG_FILTER};

#[derive(Debug, Serialize)]
pub struct Body {
    pub request: Value,
    pub response: Option<Value>,
}

fn json(text: Option<String>) -> Option<Value> {
    text.and_then(|t| serde_json::from_str(&t).ok())
}

/// The body of one request, if `user` sees its log line.
pub async fn get(pool: &DbPool, user: &User, request_id: &str) -> AppResult<Body> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT b.request, b.response FROM request_bodies b
         JOIN request_logs l ON l.request_id = b.request_id
         WHERE b.request_id = $3
           AND ($1 OR l.user_id = $2
                OR l.team_id IN (SELECT team_id FROM team_members WHERE user_id = $2))",
    )
    .bind(user.is_admin())
    .bind(user.id)
    .bind(request_id)
    .fetch_optional(pool)
    .await?;
    let (request, response) =
        row.ok_or_else(|| AppError::NotFound(format!("body of request {request_id}")))?;
    Ok(Body {
        request: json(Some(request)).unwrap_or(Value::Null),
        response: json(response),
    })
}

/// Deletes the bodies older than their key keeps them, and those of keys
/// that no longer exist. Returns how many went.
pub async fn purge(pool: &DbPool, now: DateTime<Utc>) -> AppResult<u64> {
    let retentions: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT body_retention_days FROM api_keys WHERE body_retention_days IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    let mut purged = 0;
    for days in retentions {
        let Some(before) = TimeDelta::try_days(days).and_then(|d| now.checked_sub_signed(d)) else {
            continue;
        };
        purged += sqlx::query(
            "DELETE FROM request_bodies WHERE created_at < $1
               AND key_id IN (SELECT id FROM api_keys WHERE body_retention_days = $2)",
        )
        .bind(before)
        .bind(days)
        .execute(pool)
        .await?
        .rows_affected();
    }
    purged += sqlx::query(
        "DELETE FROM request_bodies
         WHERE NOT EXISTS (SELECT 1 FROM api_keys k WHERE k.id = request_bodies.key_id)",
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(purged)
}

/// Purges every hour, for as long as the process runs.
pub fn spawn_purger(pool: DbPool) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            every.tick().await;
            match purge(&pool, Utc::now()).await {
                Ok(0) => {}
                Ok(n) => log::info!("purged {n} request bodies past their retention"),
                Err(e) => log::error!("purging request bodies failed: {e}"),
            }
        }
    });
}

/// `json`: one line per request, its log line and both bodies. `chat`:
/// OpenAI's chat format (the conversation with the reply last), for
/// fine-tuning and evals; only successful chat completions fit it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Json,
    Chat,
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    #[serde(default)]
    pub format: Format,
}

#[derive(sqlx::FromRow)]
struct ExportRow {
    id: i64,
    request_id: String,
    created_at: DateTime<Utc>,
    endpoint: String,
    model_name: String,
    provider: String,
    status_code: i32,
    key_id: i64,
    key_name: Option<String>,
    team_id: Option<i64>,
    team_name: Option<String>,
    user_id: Option<i64>,
    end_user: Option<String>,
    tags: Option<String>,
    prompt_tokens: i64,
    completion_tokens: i64,
    cached_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    cost_nanos: i64,
    latency_ms: i64,
    stream: bool,
    error: Option<String>,
    request: String,
    response: Option<String>,
}

impl ExportRow {
    fn line(self, format: Format) -> Option<Value> {
        let request = json(Some(self.request)).unwrap_or(Value::Null);
        let response = json(self.response);
        match format {
            Format::Json => Some(json!({
                "request_id": self.request_id,
                "created_at": self.created_at,
                "endpoint": self.endpoint,
                "model": self.model_name,
                "provider": self.provider,
                "status_code": self.status_code,
                "key_id": self.key_id,
                "key_name": self.key_name,
                "team_id": self.team_id,
                "team_name": self.team_name,
                "user_id": self.user_id,
                "end_user": self.end_user,
                "tags": json(self.tags).unwrap_or_else(|| json!([])),
                "prompt_tokens": self.prompt_tokens,
                "completion_tokens": self.completion_tokens,
                "cached_tokens": self.cached_tokens,
                "cache_write_tokens": self.cache_write_tokens,
                "reasoning_tokens": self.reasoning_tokens,
                "cost_usd": nanos_to_usd(self.cost_nanos),
                "latency_ms": self.latency_ms,
                "stream": self.stream,
                "error": self.error,
                "request": request,
                "response": response,
            })),
            Format::Chat => chat_example(&self.endpoint, self.status_code, &request, response?),
        }
    }
}

/// `{"messages": [...], "tools": [...]}`: the request's conversation with the
/// reply appended.
fn chat_example(endpoint: &str, status: i32, request: &Value, response: Value) -> Option<Value> {
    if endpoint != "chat/completions" || status >= 400 {
        return None;
    }
    let reply = response["choices"][0]["message"].as_object()?;
    let reply: serde_json::Map<String, Value> = reply
        .iter()
        .filter(|(k, v)| {
            matches!(
                k.as_str(),
                "role" | "content" | "tool_calls" | "function_call"
            ) && !v.is_null()
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut messages = request["messages"].as_array()?.clone();
    messages.push(Value::Object(reply));
    let mut example = json!({"messages": messages});
    if let Some(tools) = request.get("tools").filter(|t| !t.is_null()) {
        example["tools"] = tools.clone();
    }
    Some(example)
}

const PAGE: i64 = 100;

/// Every stored body behind the log lines `user` sees as filtered, newest
/// first, a line each. Read a page at a time, so any size streams.
pub async fn export(
    pool: &DbPool,
    user: &User,
    list: &ListQuery,
    query: &LogQuery,
    format: Format,
) -> AppResult<impl Stream<Item = Result<Bytes, actix_web::Error>>> {
    let filter = LogFilter::new(pool, user, list, query).await?;
    let pool = pool.clone();
    let start = (Some(i64::MAX), filter);
    Ok(futures_util::stream::unfold(
        start,
        move |(before, filter)| {
            let pool = pool.clone();
            async move {
                let before = before?;
                let sql = format!(
                    "SELECT l.id, l.request_id, l.created_at, l.endpoint, l.model_name, l.provider,
                            l.status_code, l.key_id, k.name AS key_name, l.team_id, t.name AS team_name,
                            l.user_id, l.end_user, l.tags, l.prompt_tokens, l.completion_tokens,
                            l.cached_tokens, l.cache_write_tokens, l.reasoning_tokens, l.cost_nanos,
                            l.latency_ms, l.stream, l.error, b.request, b.response
                     FROM request_logs l
                     JOIN request_bodies b ON b.request_id = l.request_id
                     LEFT JOIN teams t ON t.id = l.team_id
                     LEFT JOIN api_keys k ON k.id = l.key_id
                     WHERE {LOG_FILTER} AND l.id < $12
                     ORDER BY l.id DESC
                     LIMIT $13"
                );
                let rows = bind_log_filter!(
                    sqlx::query_as::<_, ExportRow>(sqlx::AssertSqlSafe(sql)),
                    filter
                )
                .bind(before)
                .bind(PAGE)
                .fetch_all(&pool)
                .await;
                let rows = match rows {
                    Ok(rows) => rows,
                    Err(e) => {
                        log::error!("exporting request bodies failed: {e}");
                        let error: actix_web::Error = AppError::from(e).into();
                        return Some((Err(error), (None, filter)));
                    }
                };
                let next = (rows.len() as i64 == PAGE)
                    .then(|| rows.last().map(|r| r.id))
                    .flatten();
                let mut out = Vec::new();
                for row in rows {
                    if let Some(line) = row.line(format) {
                        out.extend_from_slice(line.to_string().as_bytes());
                        out.push(b'\n');
                    }
                }
                Some((Ok(Bytes::from(out)), (next, filter)))
            }
        },
    ))
}
