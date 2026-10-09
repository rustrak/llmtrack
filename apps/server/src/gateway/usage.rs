//! Usage reaches the database off the request path, in batches.
//!
//! Requests hand their [`UsageEvent`] to a channel and return. One task drains
//! it: whatever has queued up is written in a single transaction (the log
//! rows, the daily rollup, the key and team spend, the bodies of keys that
//! keep them), so the write rate tracks
//! batches, not requests. Live budget checks never wait for it; they read the
//! in-memory counters that [`super::forward::Meter`] bumps immediately.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

use super::capture;
use crate::db::{begin_write, DbPool};

#[derive(Debug, Clone)]
pub struct UsageEvent {
    pub request_id: String,
    pub key_id: i64,
    pub team_id: Option<i64>,
    pub user_id: Option<i64>,
    /// The deployment that was called.
    pub model_id: i64,
    pub end_user: Option<String>,
    pub tags: Vec<String>,
    pub model_name: String,
    pub provider: String,
    pub status_code: u16,
    /// The OpenAI-style path it came in on (`chat/completions`, `messages`, …).
    pub endpoint: &'static str,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub cost_nanos: i64,
    pub latency_ms: i64,
    pub stream: bool,
    pub error: Option<String>,
    pub at: DateTime<Utc>,
    /// For keys that keep them.
    pub bodies: Option<Box<Bodies>>,
}

/// What was asked, and what was answered when there was an answer.
#[derive(Debug, Clone)]
pub struct Bodies {
    pub request: serde_json::Value,
    pub response: Option<serde_json::Value>,
}

pub enum Message {
    Event(Box<UsageEvent>),
    /// Answered once everything queued before it is written.
    Flush(oneshot::Sender<()>),
}

const BATCH: usize = 512;
/// Requests in flight past this wait for the writer instead of queueing more.
const QUEUE: usize = 16_384;

pub fn spawn_writer(pool: DbPool) -> mpsc::Sender<Message> {
    let (tx, rx) = mpsc::channel(QUEUE);
    tokio::spawn(run(pool, rx));
    tx
}

async fn run(pool: DbPool, mut rx: mpsc::Receiver<Message>) {
    let mut messages = Vec::with_capacity(BATCH);
    while rx.recv_many(&mut messages, BATCH).await > 0 {
        let mut events = Vec::new();
        let mut waiting = Vec::new();
        for message in messages.drain(..) {
            match message {
                Message::Event(event) => events.push(*event),
                Message::Flush(done) => waiting.push(done),
            }
        }
        if !events.is_empty() {
            if let Err(e) = write_batch(&pool, &mut events).await {
                // ponytail: a failed batch is logged and dropped; spool to disk if billing must survive a DB outage.
                log::error!("failed to write {} usage events: {e}", events.len());
            }
        }
        for done in waiting {
            let _ = done.send(());
        }
    }
}

async fn write_batch(pool: &DbPool, events: &mut [UsageEvent]) -> Result<(), sqlx::Error> {
    let mut key_totals: HashMap<i64, (i64, DateTime<Utc>)> = HashMap::new();
    let mut team_totals: HashMap<i64, i64> = HashMap::new();
    let mut tx = begin_write(pool).await?;

    for e in events.iter_mut() {
        if let Some(bodies) = e.bodies.as_mut() {
            capture::trim(&mut bodies.request);
            if let Some(response) = bodies.response.as_mut() {
                capture::trim(response);
            }
            sqlx::query(
                "INSERT INTO request_bodies (request_id, key_id, request, response, created_at)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(&e.request_id)
            .bind(e.key_id)
            .bind(bodies.request.to_string())
            .bind(bodies.response.as_ref().map(serde_json::Value::to_string))
            .bind(e.at)
            .execute(&mut *tx)
            .await?;
        }
        let e = &*e;
        sqlx::query(
            "INSERT INTO request_logs (request_id, team_id, key_id, model_name, provider,
                 status_code, prompt_tokens, completion_tokens, cost_nanos, latency_ms,
                 stream, error, created_at, cached_tokens, cache_write_tokens,
                 reasoning_tokens, endpoint, user_id, model_id, end_user, tags)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     $17, $18, $19, $20, $21)",
        )
        .bind(&e.request_id)
        .bind(e.team_id)
        .bind(e.key_id)
        .bind(&e.model_name)
        .bind(&e.provider)
        .bind(i32::from(e.status_code))
        .bind(e.prompt_tokens)
        .bind(e.completion_tokens)
        .bind(e.cost_nanos)
        .bind(e.latency_ms)
        .bind(e.stream)
        .bind(&e.error)
        .bind(e.at)
        .bind(e.cached_tokens)
        .bind(e.cache_write_tokens)
        .bind(e.reasoning_tokens)
        .bind(e.endpoint)
        .bind(e.user_id)
        .bind(e.model_id)
        .bind(&e.end_user)
        .bind((!e.tags.is_empty()).then(|| serde_json::to_string(&e.tags).unwrap_or_default()))
        .execute(&mut *tx)
        .await?;

        let failed = i64::from(e.status_code >= 400);
        sqlx::query(
            "INSERT INTO usage_daily (day, team_id, key_id, model_name, requests, failed_requests,
                 prompt_tokens, completion_tokens, cost_nanos, cached_tokens,
                 cache_write_tokens, reasoning_tokens, user_id, latency_ms, person_id)
             VALUES ($1, $2, $3, $4, 1, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                     (SELECT person_id FROM api_keys WHERE id = $3))
             ON CONFLICT (day, key_id, model_name) DO UPDATE SET
                 requests = usage_daily.requests + 1,
                 failed_requests = usage_daily.failed_requests + excluded.failed_requests,
                 prompt_tokens = usage_daily.prompt_tokens + excluded.prompt_tokens,
                 completion_tokens = usage_daily.completion_tokens + excluded.completion_tokens,
                 cost_nanos = usage_daily.cost_nanos + excluded.cost_nanos,
                 cached_tokens = usage_daily.cached_tokens + excluded.cached_tokens,
                 cache_write_tokens = usage_daily.cache_write_tokens + excluded.cache_write_tokens,
                 reasoning_tokens = usage_daily.reasoning_tokens + excluded.reasoning_tokens,
                 latency_ms = usage_daily.latency_ms + excluded.latency_ms",
        )
        .bind(e.at.format("%Y-%m-%d").to_string())
        .bind(e.team_id)
        .bind(e.key_id)
        .bind(&e.model_name)
        .bind(failed)
        .bind(e.prompt_tokens)
        .bind(e.completion_tokens)
        .bind(e.cost_nanos)
        .bind(e.cached_tokens)
        .bind(e.cache_write_tokens)
        .bind(e.reasoning_tokens)
        .bind(e.user_id)
        .bind(e.latency_ms)
        .execute(&mut *tx)
        .await?;

        let day = e.at.format("%Y-%m-%d").to_string();
        for (table, column, value) in e
            .end_user
            .iter()
            .map(|u| ("usage_daily_end_users", "end_user", u))
            .chain(e.tags.iter().map(|t| ("usage_daily_tags", "tag", t)))
        {
            // `table` and `column` are the literals above.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {table} (day, key_id, {column}, team_id, user_id, requests,
                     failed_requests, prompt_tokens, completion_tokens, cost_nanos,
                     cached_tokens, cache_write_tokens, reasoning_tokens, latency_ms)
                 VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9, $10, $11, $12, $13)
                 ON CONFLICT (day, key_id, {column}) DO UPDATE SET
                     requests = {table}.requests + 1,
                     failed_requests = {table}.failed_requests + excluded.failed_requests,
                     prompt_tokens = {table}.prompt_tokens + excluded.prompt_tokens,
                     completion_tokens = {table}.completion_tokens + excluded.completion_tokens,
                     cost_nanos = {table}.cost_nanos + excluded.cost_nanos,
                     cached_tokens = {table}.cached_tokens + excluded.cached_tokens,
                     cache_write_tokens = {table}.cache_write_tokens + excluded.cache_write_tokens,
                     reasoning_tokens = {table}.reasoning_tokens + excluded.reasoning_tokens,
                     latency_ms = {table}.latency_ms + excluded.latency_ms"
            )))
            .bind(&day)
            .bind(e.key_id)
            .bind(value)
            .bind(e.team_id)
            .bind(e.user_id)
            .bind(failed)
            .bind(e.prompt_tokens)
            .bind(e.completion_tokens)
            .bind(e.cost_nanos)
            .bind(e.cached_tokens)
            .bind(e.cache_write_tokens)
            .bind(e.reasoning_tokens)
            .bind(e.latency_ms)
            .execute(&mut *tx)
            .await?;
        }

        let key = key_totals.entry(e.key_id).or_insert((0, e.at));
        key.0 += e.cost_nanos;
        key.1 = key.1.max(e.at);
        if let Some(team_id) = e.team_id {
            *team_totals.entry(team_id).or_default() += e.cost_nanos;
        }
    }

    for (key_id, (cost, last_used)) in key_totals {
        sqlx::query(
            "UPDATE api_keys SET spend_nanos = spend_nanos + $1, last_used_at = $2 WHERE id = $3",
        )
        .bind(cost)
        .bind(last_used)
        .bind(key_id)
        .execute(&mut *tx)
        .await?;
    }
    for (team_id, cost) in team_totals {
        sqlx::query("UPDATE teams SET spend_nanos = spend_nanos + $1 WHERE id = $2")
            .bind(cost)
            .bind(team_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}
