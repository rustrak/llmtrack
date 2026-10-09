//! Request and response bodies: stored only for keys that opt in, kept as
//! long as the key says, readable by whoever sees the log, exportable.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use chrono::{Duration, Utc};
use serde_json::{json, Value};

async fn setup<S>(app: &mut TestApp<S>, upstream: &Upstream, key: Value) -> (i64, String)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.admin().await;
    for (name, provider) in [("gpt", "openai_compatible"), ("claude", "anthropic")] {
        let (status, body) = app
            .call(
                "POST",
                "/api/models",
                Some(json!({"name": name, "provider": provider,
                "upstream_model": name, "api_base": upstream.base,
                "pricing": {"input": 2.5, "output": 10.0}})),
            )
            .await;
        assert_eq!(status, 201, "{body}");
    }
    let (status, key) = app.call("POST", "/api/keys", Some(key)).await;
    assert_eq!(status, 201, "{key}");
    (
        key["id"].as_i64().unwrap(),
        key["key"].as_str().unwrap().into(),
    )
}

async fn send<S>(app: &mut TestApp<S>, key: &str, path: &str, body: Value) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    let (status, text) = app
        .call_text(path, body, &[("Authorization", auth.as_str())])
        .await;
    assert_eq!(status, 200, "{text}");
    text
}

/// The newest log line, once the writer has caught up.
async fn last_log<S>(app: &mut TestApp<S>) -> Value
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    logs["data"][0].clone()
}

async fn body_of<S>(app: &mut TestApp<S>, log: &Value) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let uri = format!("/api/logs/{}/body", log["request_id"].as_str().unwrap());
    app.call("GET", &uri, None).await
}

async fn last_body<S>(app: &mut TestApp<S>) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let log = last_log(app).await;
    body_of(app, &log).await
}

fn hi(model: &str) -> Value {
    json!({"model": model, "messages": [{"role": "user", "content": "hi"}]})
}

#[actix_web::test]
async fn keys_do_not_store_bodies_unless_they_opt_in() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (id, key) = setup(&mut app, &upstream, json!({"name": "k"})).await;
    let (_, info) = app.call("GET", &format!("/api/keys/{id}"), None).await;
    assert_eq!(info["log_bodies"], false);
    assert_eq!(info["body_retention_days"], Value::Null);

    send(&mut app, &key, "/v1/chat/completions", hi("gpt")).await;
    let log = last_log(&mut app).await;
    assert_eq!(log["has_body"], false);
    assert_eq!(body_of(&mut app, &log).await.0, 404);

    // Switched on: the next request is kept, no restart needed.
    let (status, info) = app
        .call(
            "PATCH",
            &format!("/api/keys/{id}"),
            Some(json!({"log_bodies": true, "body_retention_days": 30})),
        )
        .await;
    assert_eq!(status, 200, "{info}");
    assert_eq!(info["log_bodies"], true);
    assert_eq!(info["body_retention_days"], 30);
    send(&mut app, &key, "/v1/chat/completions", hi("gpt")).await;
    let log = last_log(&mut app).await;
    assert_eq!(log["has_body"], true);
    let (status, body) = body_of(&mut app, &log).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["request"], hi("gpt"), "what the client sent");
    assert_eq!(
        body["response"]["choices"][0]["message"]["content"],
        "Hello world"
    );
}

#[actix_web::test]
async fn retention_must_be_a_positive_number_of_days() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (id, _) = setup(&mut app, &upstream, json!({"name": "k"})).await;
    let (status, _) = app
        .call(
            "PATCH",
            &format!("/api/keys/{id}"),
            Some(json!({"body_retention_days": 0})),
        )
        .await;
    assert_eq!(status, 400);
    let (status, info) = app
        .call(
            "PATCH",
            &format!("/api/keys/{id}"),
            Some(json!({"body_retention_days": null})),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(
        info["body_retention_days"],
        Value::Null,
        "null keeps forever"
    );
}

#[actix_web::test]
async fn streamed_replies_are_stored_whole_in_the_dialect_the_client_speaks() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (_, key) = setup(
        &mut app,
        &upstream,
        json!({"name": "k", "log_bodies": true}),
    )
    .await;

    // OpenAI's own stream: chunks folded into one completion.
    let mut chat = hi("gpt");
    chat["stream"] = json!(true);
    send(&mut app, &key, "/v1/chat/completions", chat.clone()).await;
    let (_, body) = last_body(&mut app).await;
    assert_eq!(body["request"], chat);
    let reply = &body["response"];
    assert_eq!(reply["object"], "chat.completion");
    assert_eq!(reply["choices"][0]["message"]["role"], "assistant");
    assert_eq!(reply["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(reply["choices"][0]["finish_reason"], "stop");

    // Anthropic translated to chat chunks.
    chat["model"] = json!("claude");
    send(&mut app, &key, "/v1/chat/completions", chat).await;
    let (_, body) = last_body(&mut app).await;
    assert_eq!(
        body["response"]["choices"][0]["message"]["content"],
        "Hello world"
    );

    // Anthropic's own stream: events folded into one message.
    let messages = json!({"model": "claude", "max_tokens": 5, "stream": true,
        "messages": [{"role": "user", "content": "hi"}]});
    send(&mut app, &key, "/v1/messages", messages).await;
    let (_, body) = last_body(&mut app).await;
    let reply = &body["response"];
    assert_eq!(reply["type"], "message");
    assert_eq!(reply["content"][0]["text"], "Hello world");
    assert_eq!(reply["stop_reason"], "end_turn");

    // The Responses API: the final event carries the whole response.
    let responses = json!({"model": "claude", "input": "hi", "stream": true});
    send(&mut app, &key, "/v1/responses", responses).await;
    let (_, body) = last_body(&mut app).await;
    assert_eq!(body["response"]["object"], "response");
    assert_eq!(
        body["response"]["output"][0]["content"][0]["text"],
        "Hello world"
    );
}

#[actix_web::test]
async fn a_failed_request_keeps_what_was_asked_and_what_the_provider_said() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (_, key) = setup(
        &mut app,
        &upstream,
        json!({"name": "k", "log_bodies": true}),
    )
    .await;
    let (status, _) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "broken", "provider": "openai_compatible",
                "upstream_model": "too-long", "api_base": upstream.base})),
        )
        .await;
    assert_eq!(status, 201);
    let auth = format!("Bearer {key}");
    let (status, _) = app
        .call_with(
            "POST",
            "/v1/chat/completions",
            Some(hi("broken")),
            &[("Authorization", auth.as_str())],
        )
        .await;
    assert_eq!(status, 400);
    let log = last_log(&mut app).await;
    let (_, body) = body_of(&mut app, &log).await;
    assert_eq!(body["request"], hi("broken"));
    assert!(body["response"]["error"].is_object(), "{body}");
}

#[actix_web::test]
async fn bodies_go_once_the_key_retention_passes() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (_, short) = setup(
        &mut app,
        &upstream,
        json!({"name": "short", "log_bodies": true, "body_retention_days": 7}),
    )
    .await;
    let (_, forever) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"name": "forever", "log_bodies": true})),
        )
        .await;
    let forever = forever["key"].as_str().unwrap().to_string();
    send(&mut app, &short, "/v1/chat/completions", hi("gpt")).await;
    send(&mut app, &forever, "/v1/chat/completions", hi("gpt")).await;
    app.state.gateway.flush().await;

    let pool = app.pool().clone();
    let purged = llmtrack::services::bodies::purge(&pool, Utc::now() + Duration::days(6))
        .await
        .unwrap();
    assert_eq!(purged, 0, "not old enough yet");
    let purged = llmtrack::services::bodies::purge(&pool, Utc::now() + Duration::days(8))
        .await
        .unwrap();
    assert_eq!(purged, 1, "only the key with a retention loses its bodies");

    let (_, logs) = app.call("GET", "/api/logs", None).await;
    let kept: Vec<(&str, bool)> = logs["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["key_name"].as_str().unwrap(),
                l["has_body"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(kept, [("forever", true), ("short", false)]);
    assert_eq!(logs["total"], 2, "the log lines themselves stay");
}

#[actix_web::test]
async fn bodies_are_seen_by_whoever_sees_the_log() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (_, key) = setup(
        &mut app,
        &upstream,
        json!({"name": "k", "log_bodies": true}),
    )
    .await;
    send(&mut app, &key, "/v1/chat/completions", hi("gpt")).await;
    let log = last_log(&mut app).await;

    app.login_as("stranger@example.com", "member").await;
    assert_eq!(body_of(&mut app, &log).await.0, 404);
    let (status, _, text) = app.call_bytes("/api/logs/export", &[]).await;
    assert_eq!(status, 200);
    assert!(text.is_empty(), "nothing of anyone else's");
}

#[actix_web::test]
async fn bodies_export_as_json_lines() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (_, key) = setup(
        &mut app,
        &upstream,
        json!({"name": "k", "log_bodies": true}),
    )
    .await;
    let mut tooled = hi("gpt");
    tooled["tools"] = json!([{"type": "function", "function": {"name": "f"}}]);
    send(&mut app, &key, "/v1/chat/completions", tooled.clone()).await;
    let messages = json!({"model": "claude", "max_tokens": 5,
        "messages": [{"role": "user", "content": "hi"}]});
    send(&mut app, &key, "/v1/messages", messages.clone()).await;
    app.state.gateway.flush().await;

    // Every stored request, metadata alongside, newest first.
    let (status, headers, bytes) = app.call_bytes("/api/logs/export", &[]).await;
    assert_eq!(status, 200);
    assert_eq!(headers.get("content-type").unwrap(), "application/x-ndjson");
    assert!(headers
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .ends_with(".jsonl\""));
    let lines: Vec<Value> = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["endpoint"], "messages");
    assert_eq!(lines[0]["request"], messages);
    assert_eq!(lines[0]["response"]["content"][0]["text"], "Hello world");
    assert_eq!(lines[1]["model"], "gpt");
    assert_eq!(lines[1]["key_name"], "k");
    assert_eq!(lines[1]["prompt_tokens"], 10);
    assert!(lines[1]["cost_usd"].as_f64().unwrap() > 0.0);
    assert!(lines[1]["request_id"].is_string());

    // The log filters apply.
    let (_, _, bytes) = app.call_bytes("/api/logs/export?model=claude", &[]).await;
    assert_eq!(String::from_utf8(bytes).unwrap().lines().count(), 1);

    // OpenAI's chat format, for fine-tuning and evals: the conversation with
    // the reply last. Other dialects have no place in it.
    let (status, headers, bytes) = app.call_bytes("/api/logs/export?format=chat", &[]).await;
    assert_eq!(status, 200);
    assert!(headers
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .ends_with(".jsonl\""));
    let lines: Vec<Value> = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines,
        [json!({"messages": [{"role": "user", "content": "hi"},
                             {"role": "assistant", "content": "Hello world"}],
                "tools": tooled["tools"]})]
    );

    let (status, _, _) = app.call_bytes("/api/logs/export?format=csv", &[]).await;
    assert_eq!(status, 400);
}
