//! The gateway end to end: virtual key in, provider call out, spend recorded.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

/// Registers a model on the fake upstream ($2.50 in / $10 out per 1M tokens),
/// a team and a key, and returns the raw key.
async fn key_for<S>(
    app: &mut TestApp<S>,
    upstream: &Upstream,
    provider: &str,
    name: &str,
    upstream_model: &str,
) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    if app.cookie.is_none() {
        app.admin().await;
    }
    register_model(app, upstream, provider, name, upstream_model).await;
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": format!("team-{name}"), "all_models": true})),
        )
        .await;
    let (status, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "app"})),
        )
        .await;
    assert_eq!(status, 201, "{key}");
    key["key"].as_str().unwrap().to_string()
}

async fn register_model<S>(
    app: &mut TestApp<S>,
    upstream: &Upstream,
    provider: &str,
    name: &str,
    upstream_model: &str,
) -> Value
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({
                "name": name,
                "provider": provider,
                "upstream_model": upstream_model,
                "api_base": upstream.base,
                "api_key": "provider-secret",
                "pricing": {"input": 2.5, "output": 10.0}
            })),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    model
}

fn chat(model: &str) -> Value {
    json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "Be terse."},
            {"role": "user", "content": "Say hello"}
        ]
    })
}

fn bearer(key: &str) -> (&'static str, String) {
    ("Authorization", format!("Bearer {key}"))
}

async fn post<S>(app: &mut TestApp<S>, uri: &str, key: &str, body: Value) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (name, value) = bearer(key);
    app.call_with("POST", uri, Some(body), &[(name, &value)])
        .await
}

#[derive(Debug, sqlx::FromRow)]
struct LogRow {
    model_name: String,
    provider: String,
    status_code: i32,
    prompt_tokens: i64,
    completion_tokens: i64,
    cost_nanos: i64,
    stream: bool,
    error: Option<String>,
}

async fn logs<S>(app: &TestApp<S>) -> Vec<LogRow> {
    app.state.gateway.flush().await;
    sqlx::query_as::<_, LogRow>(
        "SELECT model_name, provider, status_code, prompt_tokens, completion_tokens, cost_nanos,
                stream, error FROM request_logs ORDER BY id",
    )
    .fetch_all(&app.db.pool)
    .await
    .unwrap()
}

/// 10 prompt tokens at $2.50/M plus 20 completion tokens at $10/M.
const EXPECTED_COST_NANOS: i64 = 10 * 2_500 + 20 * 10_000;

#[actix_web::test]
async fn a_chat_completion_is_proxied_and_billed() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o-2024-08-06").await;

    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o")).await;

    assert_eq!(status, 200, "{body}");
    assert_eq!(body["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(body["usage"]["total_tokens"], 30);

    let sent = upstream.last();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.body["model"], "gpt-4o-2024-08-06",
        "the upstream name is sent"
    );
    assert_eq!(
        sent.authorization.as_deref(),
        Some("Bearer provider-secret")
    );
    assert_eq!(sent.body["messages"][1]["content"], "Say hello");

    let logs = logs(&app).await;
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model_name, "gpt-4o");
    assert_eq!(logs[0].provider, "openai");
    assert_eq!(logs[0].status_code, 200);
    assert_eq!((logs[0].prompt_tokens, logs[0].completion_tokens), (10, 20));
    assert_eq!(logs[0].cost_nanos, EXPECTED_COST_NANOS);
    assert!(!logs[0].stream);
    let (logged, rolled): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT latency_ms FROM request_logs), (SELECT latency_ms FROM usage_daily)",
    )
    .fetch_one(&app.db.pool)
    .await
    .unwrap();
    assert_eq!(rolled, logged, "the rollup sums latency for averages");

    let (_, keys) = app.call("GET", "/api/keys", None).await;
    assert_eq!(
        keys["data"][0]["spend_usd"],
        EXPECTED_COST_NANOS as f64 / 1e9
    );
    assert!(keys["data"][0]["last_used_at"].is_string());
    let (_, teams) = app.call("GET", "/api/teams", None).await;
    assert_eq!(
        teams["data"][0]["spend_usd"],
        EXPECTED_COST_NANOS as f64 / 1e9
    );
}

#[actix_web::test]
async fn the_unversioned_path_and_x_api_key_also_work() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;

    let (status, _) = app
        .call_with(
            "POST",
            "/chat/completions",
            Some(chat("gpt-4o")),
            &[("x-api-key", &key)],
        )
        .await;
    assert_eq!(status, 200);
}

#[actix_web::test]
async fn missing_and_unknown_keys_are_401_in_openai_shape() {
    let mut app = test_app().await;

    let (status, body) = app
        .call_with("POST", "/v1/chat/completions", Some(chat("gpt-4o")), &[])
        .await;
    assert_eq!(status, 401);
    assert!(body["error"]["message"].is_string());

    let (status, _) = post(
        &mut app,
        "/v1/chat/completions",
        "sk-not-a-real-key",
        chat("gpt-4o"),
    )
    .await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn a_dashboard_session_is_not_an_api_key() {
    let mut app = test_app().await;
    app.admin().await;
    let (status, _) = app
        .call_with("POST", "/v1/chat/completions", Some(chat("gpt-4o")), &[])
        .await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn unknown_and_disallowed_models_are_refused_before_any_provider_call() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let allowed = register_model(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    register_model(&mut app, &upstream, "openai", "o3", "o3").await;
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "Acme", "models": [allowed["id"]]})),
        )
        .await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "app"})),
        )
        .await;
    let key = key["key"].as_str().unwrap().to_string();

    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("nope")).await;
    assert_eq!(status, 404, "{body}");
    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("o3")).await;
    assert_eq!(status, 403, "{body}");
    let (status, _) = post(
        &mut app,
        "/v1/chat/completions",
        &key,
        json!({"messages": []}),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(upstream.count(), 0);
}

#[actix_web::test]
async fn models_lists_what_the_key_may_call() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let gpt = register_model(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    register_model(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
    )
    .await;
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "Acme", "models": [gpt["id"]]})),
        )
        .await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "app"})),
        )
        .await;
    let (name, value) = bearer(key["key"].as_str().unwrap());

    let (status, body) = app
        .call_with("GET", "/v1/models", None, &[(name, &value)])
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["object"], "list");
    let ids: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["gpt-4o"]);
}

#[actix_web::test]
async fn a_key_over_budget_is_stopped_with_429() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    // One request costs $0.000225; a budget of $0.0001 allows exactly one.
    let (_, keys) = app.call("GET", "/api/keys", None).await;
    app.call(
        "PATCH",
        &format!("/api/keys/{}", keys["data"][0]["id"]),
        Some(json!({"max_budget_usd": 0.0001})),
    )
    .await;

    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o"))
            .await
            .0,
        200
    );
    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o")).await;
    assert_eq!(status, 429);
    assert_eq!(body["error"]["type"], "insufficient_quota");
    assert_eq!(upstream.count(), 1);
}

#[actix_web::test]
async fn a_team_over_budget_stops_every_key_of_the_team() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    let (_, teams) = app.call("GET", "/api/teams", None).await;
    let team_id = teams["data"][0]["id"].clone();
    app.call(
        "PATCH",
        &format!("/api/teams/{team_id}"),
        Some(json!({"max_budget_usd": 0.0001})),
    )
    .await;
    let (_, second) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team_id, "name": "second"})),
        )
        .await;

    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o"))
            .await
            .0,
        200
    );
    let second = second["key"].as_str().unwrap().to_string();
    assert_eq!(
        post(&mut app, "/v1/chat/completions", &second, chat("gpt-4o"))
            .await
            .0,
        429
    );
}

#[actix_web::test]
async fn revoked_and_expired_keys_stop_working_at_once() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o"))
            .await
            .0,
        200
    );

    let (_, keys) = app.call("GET", "/api/keys", None).await;
    app.call(
        "DELETE",
        &format!("/api/keys/{}", keys["data"][0]["id"]),
        None,
    )
    .await;
    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o"))
            .await
            .0,
        401
    );

    let (_, teams) = app.call("GET", "/api/teams", None).await;
    let (_, expiring) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": teams["data"][0]["id"], "name": "soon"})),
        )
        .await;
    sqlx::query("UPDATE api_keys SET expires_at = $1 WHERE id = $2")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .bind(expiring["id"].as_i64().unwrap())
        .execute(app.pool())
        .await
        .unwrap();
    let (status, body) = post(
        &mut app,
        "/v1/chat/completions",
        expiring["key"].as_str().unwrap(),
        chat("gpt-4o"),
    )
    .await;
    assert_eq!(status, 401);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("expired"));
}

#[actix_web::test]
async fn a_model_added_or_disabled_later_is_picked_up() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("later"))
            .await
            .0,
        404
    );

    let later = register_model(&mut app, &upstream, "openai", "later", "later").await;
    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("later"))
            .await
            .0,
        200
    );

    app.call(
        "PATCH",
        &format!("/api/models/{}", later["id"]),
        Some(json!({"is_active": false})),
    )
    .await;
    assert_eq!(
        post(&mut app, "/v1/chat/completions", &key, chat("later"))
            .await
            .0,
        404
    );
}

#[actix_web::test]
async fn a_stream_is_passed_through_and_its_usage_billed() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    let mut body = chat("gpt-4o");
    body["stream"] = json!(true);

    let (name, value) = bearer(&key);
    let (status, text) = app
        .call_text("/v1/chat/completions", body, &[(name, &value)])
        .await;

    assert_eq!(status, 200);
    assert!(text.contains("\"Hello\""));
    assert!(text.trim_end().ends_with("data: [DONE]"));
    assert!(
        !text.contains("\"usage\""),
        "the usage chunk the client did not ask for is dropped"
    );
    assert_eq!(
        upstream.last().body["stream_options"]["include_usage"],
        true,
        "usage is requested from the provider anyway"
    );

    let logs = logs(&app).await;
    assert!(logs[0].stream);
    assert_eq!((logs[0].prompt_tokens, logs[0].completion_tokens), (10, 20));
    assert_eq!(logs[0].cost_nanos, EXPECTED_COST_NANOS);
}

#[actix_web::test]
async fn a_client_asking_for_stream_usage_gets_it() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    let mut body = chat("gpt-4o");
    body["stream"] = json!(true);
    body["stream_options"] = json!({"include_usage": true});

    let (name, value) = bearer(&key);
    let (_, text) = app
        .call_text("/v1/chat/completions", body, &[(name, &value)])
        .await;
    assert!(text.contains("\"usage\""));
}

#[actix_web::test]
async fn anthropic_is_translated_both_ways() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
    )
    .await;

    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("claude")).await;

    assert_eq!(status, 200, "{body}");
    assert_eq!(body["object"], "chat.completion");
    assert_eq!(body["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
    assert_eq!(body["usage"]["prompt_tokens"], 10);
    assert_eq!(body["usage"]["completion_tokens"], 20);

    let sent = upstream.last();
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.api_key.as_deref(), Some("provider-secret"));
    assert!(sent.anthropic_version.is_some());
    assert_eq!(sent.body["model"], "claude-sonnet-4-5");
    assert_eq!(sent.body["system"], "Be terse.");
    assert_eq!(sent.body["messages"].as_array().unwrap().len(), 1);
    assert!(sent.body["max_tokens"].as_u64().unwrap() > 0);

    let logs = logs(&app).await;
    assert_eq!(logs[0].provider, "anthropic");
    assert_eq!(logs[0].cost_nanos, EXPECTED_COST_NANOS);
}

#[actix_web::test]
async fn an_anthropic_stream_comes_back_as_openai_chunks() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
    )
    .await;
    let mut body = chat("claude");
    body["stream"] = json!(true);

    let (name, value) = bearer(&key);
    let (status, text) = app
        .call_text("/v1/chat/completions", body, &[(name, &value)])
        .await;

    assert_eq!(status, 200);
    let chunks: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| *data != "[DONE]")
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    let content: String = chunks
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(content, "Hello world");
    assert!(chunks
        .iter()
        .all(|c| c["object"] == "chat.completion.chunk"));
    assert!(chunks
        .iter()
        .any(|c| c["choices"][0]["finish_reason"] == "stop"));
    assert!(text.trim_end().ends_with("data: [DONE]"));

    let logs = logs(&app).await;
    assert_eq!((logs[0].prompt_tokens, logs[0].completion_tokens), (10, 20));
    assert!(logs[0].stream);
}

#[actix_web::test]
async fn provider_errors_are_logged_and_a_bad_provider_key_is_a_502() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "flaky", "fail-500").await;
    register_model(&mut app, &upstream, "openai", "misconfigured", "bad-key").await;

    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("flaky")).await;
    assert_eq!(status, 500);
    assert_eq!(body["error"]["message"], "the provider fell over");

    let (status, body) = post(
        &mut app,
        "/v1/chat/completions",
        &key,
        chat("misconfigured"),
    )
    .await;
    assert_eq!(status, 502, "the client's key is fine; ours is not");
    assert!(body["error"]["message"].as_str().unwrap().contains("401"));

    let logs = logs(&app).await;
    assert_eq!(logs.len(), 4, "a 500 is retried twice; a 401 is not");
    assert_eq!(upstream.count(), 4);
    assert_eq!(logs[0].status_code, 500);
    assert_eq!(logs[0].cost_nanos, 0);
    assert!(logs[0].error.is_some());
}

#[actix_web::test]
async fn an_unreachable_provider_is_a_502() {
    let mut app = test_app().await;
    app.admin().await;
    let dead = Upstream {
        base: "http://127.0.0.1:9/v1".into(),
        requests: Default::default(),
    };
    let key = key_for(&mut app, &dead, "openai", "gpt-4o", "gpt-4o").await;

    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o")).await;
    assert_eq!(status, 502, "{body}");
    assert_eq!(logs(&app).await[0].status_code, 502);
}

#[actix_web::test]
async fn embeddings_are_proxied_and_billed_at_the_input_price() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(
        &mut app,
        &upstream,
        "openai",
        "embed",
        "text-embedding-3-small",
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/embeddings",
        &key,
        json!({"model": "embed", "input": "hi"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(upstream.last().path, "/v1/embeddings");
    assert_eq!(upstream.last().body["model"], "text-embedding-3-small");

    let logs = logs(&app).await;
    assert_eq!((logs[0].prompt_tokens, logs[0].completion_tokens), (8, 0));
    assert_eq!(logs[0].cost_nanos, 8 * 2_500);
}

#[actix_web::test]
async fn usage_rolls_up_by_day() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = key_for(&mut app, &upstream, "openai", "gpt-4o", "gpt-4o").await;
    for _ in 0..3 {
        post(&mut app, "/v1/chat/completions", &key, chat("gpt-4o")).await;
    }
    app.state.gateway.flush().await;

    let (requests, cost, day): (i64, i64, String) =
        sqlx::query_as("SELECT requests, cost_nanos, day FROM usage_daily")
            .fetch_one(app.pool())
            .await
            .unwrap();
    assert_eq!(requests, 3);
    assert_eq!(cost, 3 * EXPECTED_COST_NANOS);
    assert_eq!(day, chrono::Utc::now().format("%Y-%m-%d").to_string());
}

#[actix_web::test]
async fn azure_openai_uses_its_deployment_url_and_api_key_header() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let root = upstream.base.trim_end_matches("/v1").to_string();
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({
                "name": "azure-4o",
                "provider": "azure",
                "upstream_model": "my-gpt4o-deployment",
                "api_base": root,
                "api_version": "2024-10-21",
                "api_key": "azure-secret",
                "pricing": {"input": 2.5, "output": 10.0}
            })),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    assert_eq!(model["api_version"], "2024-10-21");
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "Acme", "all_models": true})),
        )
        .await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "app"})),
        )
        .await;

    let (status, body) = post(
        &mut app,
        "/v1/chat/completions",
        key["key"].as_str().unwrap(),
        chat("azure-4o"),
    )
    .await;
    assert_eq!(status, 200, "{body}");

    let sent = upstream.last();
    assert_eq!(
        sent.path,
        "/openai/deployments/my-gpt4o-deployment/chat/completions"
    );
    assert_eq!(sent.query, "api-version=2024-10-21");
    assert_eq!(sent.azure_key.as_deref(), Some("azure-secret"));
    assert_eq!(sent.authorization, None);
    assert_eq!(logs(&app).await[0].cost_nanos, EXPECTED_COST_NANOS);
}

#[actix_web::test]
async fn azure_needs_an_api_base_and_version() {
    let mut app = test_app().await;
    app.admin().await;
    let (status, body) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "az", "provider": "azure", "upstream_model": "d", "api_base": "https://x.openai.azure.com"})),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["fields"][0]["field"], "api_version");
}
