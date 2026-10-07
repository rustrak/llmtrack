//! The router: several deployments per model name, retries,
//! cooldowns and fallbacks.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::header::HeaderMap;
use serde_json::{json, Value};

/// A deployment of `name` on `upstream`, calling `upstream_model`.
async fn deploy<S>(
    app: &mut TestApp<S>,
    upstream: &Upstream,
    name: &str,
    upstream_model: &str,
    weight: Option<i64>,
) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": name, "provider": "openai_compatible",
                        "upstream_model": upstream_model, "api_base": upstream.base,
                        "weight": weight, "pricing": {"input": 2.5, "output": 10.0}})),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    model["id"].as_i64().unwrap()
}

/// An admin with a key that may call every model.
async fn setup<S>(app: &mut TestApp<S>) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (_, key) = app
        .call("POST", "/api/keys", Some(json!({"name": "k"})))
        .await;
    key["key"].as_str().unwrap().to_string()
}

async fn router<S>(app: &mut TestApp<S>, settings: Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, body) = app
        .call("PUT", "/api/router-settings", Some(settings))
        .await;
    assert_eq!(status, 200, "{body}");
}

async fn chat<S>(app: &mut TestApp<S>, key: &str, model: &str) -> (u16, HeaderMap, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    app.call_full(
        "POST",
        "/v1/chat/completions",
        Some(json!({"model": model, "messages": [{"role": "user", "content": "hi"}]})),
        &[("Authorization", &auth)],
    )
    .await
}

fn header(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default()
}

#[actix_web::test]
async fn traffic_is_shared_between_deployments_of_a_name() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    let first = deploy(&mut app, &a, "gpt", "gpt-4o", None).await;
    let second = deploy(&mut app, &b, "gpt", "gpt-4o", None).await;
    let key = setup(&mut app).await;

    let mut seen = std::collections::HashSet::new();
    for _ in 0..40 {
        let (status, headers, _) = chat(&mut app, &key, "gpt").await;
        assert_eq!(status, 200);
        assert_eq!(header(&headers, "x-litellm-model-group"), "gpt");
        seen.insert(header(&headers, "x-litellm-model-id"));
    }
    assert!(
        a.count() > 0 && b.count() > 0,
        "a={} b={}",
        a.count(),
        b.count()
    );
    assert_eq!(seen, [first, second].map(|id| id.to_string()).into());

    let auth = format!("Bearer {key}");
    let (_, list) = app
        .call_with("GET", "/v1/models", None, &[("Authorization", &auth)])
        .await;
    assert_eq!(
        list["data"].as_array().unwrap().len(),
        1,
        "one entry per name: {list}"
    );
}

#[actix_web::test]
async fn weights_steer_traffic() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &a, "gpt", "gpt-4o", Some(5)).await;
    deploy(&mut app, &b, "gpt", "gpt-4o", Some(0)).await;
    let key = setup(&mut app).await;
    for _ in 0..20 {
        assert_eq!(chat(&mut app, &key, "gpt").await.0, 200);
    }
    assert_eq!((a.count(), b.count()), (20, 0));
}

#[actix_web::test]
async fn a_failing_deployment_is_retried_on_another_and_cools_down() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &a, "gpt", "fail-500", Some(1)).await;
    let healthy = deploy(&mut app, &b, "gpt", "gpt-4o", Some(0)).await;
    router(&mut app, json!({"cooldown_time": 1})).await;
    let key = setup(&mut app).await;

    let (status, headers, _) = chat(&mut app, &key, "gpt").await;
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "x-litellm-attempted-retries"), "1");
    assert_eq!(header(&headers, "x-litellm-model-id"), healthy.to_string());
    for _ in 0..9 {
        assert_eq!(chat(&mut app, &key, "gpt").await.0, 200);
    }
    // Allowed 3 fails a minute; the 4th puts it in cooldown.
    assert_eq!(a.count(), 4);
    assert_eq!(b.count(), 10);

    actix_web::rt::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(chat(&mut app, &key, "gpt").await.0, 200);
    assert_eq!(a.count(), 5, "out of cooldown, it is tried again");

    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs?status=error", None).await;
    assert_eq!(
        logs["data"].as_array().unwrap().len(),
        5,
        "every failed attempt is logged"
    );
    assert!(logs["data"][0]["model_id"].is_number());
}

#[actix_web::test]
async fn a_rate_limited_deployment_cools_down_at_once() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &a, "gpt", "rate-limited", Some(1)).await;
    deploy(&mut app, &b, "gpt", "gpt-4o", Some(0)).await;
    let key = setup(&mut app).await;
    for _ in 0..3 {
        assert_eq!(chat(&mut app, &key, "gpt").await.0, 200);
    }
    assert_eq!(a.count(), 1);
}

#[actix_web::test]
async fn when_every_deployment_cools_down_the_answer_is_429() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &a, "gpt", "rate-limited", None).await;
    deploy(&mut app, &b, "gpt", "rate-limited", None).await;
    let key = setup(&mut app).await;

    assert_eq!(chat(&mut app, &key, "gpt").await.0, 429);
    let (status, _, body) = chat(&mut app, &key, "gpt").await;
    assert_eq!(status, 429);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("No deployments available"),
        "{body}"
    );
    assert_eq!(
        a.count() + b.count(),
        2,
        "nothing is sent while they cool down"
    );
}

#[actix_web::test]
async fn a_single_deployment_is_retried_and_never_cooled_down() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "flaky", "flaky-once", None).await;
    let key = setup(&mut app).await;

    let (status, headers, _) = chat(&mut app, &key, "flaky").await;
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "x-litellm-attempted-retries"), "1");
    assert_eq!(upstream.count(), 2);
}

#[actix_web::test]
async fn num_retries_is_a_setting() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "flaky", "flaky-once", None).await;
    router(&mut app, json!({"num_retries": 0})).await;
    let key = setup(&mut app).await;

    assert_eq!(chat(&mut app, &key, "flaky").await.0, 500);
    assert_eq!(upstream.count(), 1);
}

#[actix_web::test]
async fn client_errors_are_not_retried() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "small", "too-long", None).await;
    deploy(&mut app, &upstream, "small", "too-long", None).await;
    let key = setup(&mut app).await;

    let (status, _, body) = chat(&mut app, &key, "small").await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "context_length_exceeded");
    assert_eq!(upstream.count(), 1);
}

#[actix_web::test]
async fn fallbacks_take_over_when_a_model_keeps_failing() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "primary", "fail-500", None).await;
    deploy(&mut app, &upstream, "backup", "gpt-4o", None).await;
    router(
        &mut app,
        json!({"num_retries": 0, "fallbacks": [{"primary": ["backup"]}]}),
    )
    .await;
    let key = setup(&mut app).await;

    let (status, headers, body) = chat(&mut app, &key, "primary").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(header(&headers, "x-litellm-attempted-fallbacks"), "1");
    assert_eq!(header(&headers, "x-litellm-model-group"), "backup");
    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(
        logs["data"][0]["model_name"], "backup",
        "billed to what answered"
    );
    assert_eq!(logs["data"][1]["model_name"], "primary");
}

#[actix_web::test]
async fn a_generic_fallback_catches_every_model() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "primary", "fail-500", None).await;
    deploy(&mut app, &upstream, "backup", "gpt-4o", None).await;
    router(
        &mut app,
        json!({"num_retries": 0, "fallbacks": [{"*": ["backup"]}]}),
    )
    .await;
    let key = setup(&mut app).await;
    assert_eq!(chat(&mut app, &key, "primary").await.0, 200);
}

#[actix_web::test]
async fn context_window_and_content_policy_errors_have_their_own_fallbacks() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    for (name, model) in [
        ("small", "too-long"),
        ("large", "gpt-4o"),
        ("strict", "unsafe"),
        ("lenient", "gpt-4o"),
    ] {
        deploy(&mut app, &upstream, name, model, None).await;
    }
    router(
        &mut app,
        json!({"context_window_fallbacks": [{"small": ["large"]}],
               "content_policy_fallbacks": [{"strict": ["lenient"]}]}),
    )
    .await;
    let key = setup(&mut app).await;

    let (status, headers, _) = chat(&mut app, &key, "small").await;
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "x-litellm-model-group"), "large");
    let (status, headers, _) = chat(&mut app, &key, "strict").await;
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "x-litellm-model-group"), "lenient");
}

#[actix_web::test]
async fn without_their_own_list_context_window_errors_use_the_general_fallbacks() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "small", "too-long", None).await;
    deploy(&mut app, &upstream, "large", "gpt-4o", None).await;
    router(&mut app, json!({"fallbacks": [{"small": ["large"]}]})).await;
    let key = setup(&mut app).await;
    assert_eq!(chat(&mut app, &key, "small").await.0, 200);
}

#[actix_web::test]
async fn fallbacks_respect_what_the_key_may_call() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let primary = deploy(&mut app, &upstream, "primary", "fail-500", None).await;
    deploy(&mut app, &upstream, "backup", "gpt-4o", None).await;
    router(
        &mut app,
        json!({"num_retries": 0, "fallbacks": [{"primary": ["backup"]}]}),
    )
    .await;
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "T", "models": [primary]})),
        )
        .await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "k"})),
        )
        .await;

    let (status, _, _) = chat(&mut app, key["key"].as_str().unwrap(), "primary").await;
    assert_eq!(status, 500, "the key may not call the fallback");
    assert_eq!(upstream.count(), 1);
}

#[actix_web::test]
async fn a_stream_is_retried_before_its_first_byte() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &a, "gpt", "fail-500", Some(1)).await;
    deploy(&mut app, &b, "gpt", "gpt-4o", Some(0)).await;
    let key = setup(&mut app).await;
    let auth = format!("Bearer {key}");
    let (status, text) = app
        .call_text(
            "/v1/chat/completions",
            json!({"model": "gpt", "stream": true, "messages": [{"role": "user", "content": "hi"}]}),
            &[("Authorization", &auth)],
        )
        .await;
    assert_eq!(status, 200);
    assert!(text.contains("[DONE]"), "{text}");
}

#[actix_web::test]
async fn other_endpoints_are_routed_too() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &a, "gpt", "fail-500", Some(1)).await;
    deploy(&mut app, &b, "gpt", "gpt-4o", Some(0)).await;
    let key = setup(&mut app).await;
    let auth = format!("Bearer {key}");
    let (status, body) = app
        .call_with(
            "POST",
            "/v1/messages",
            Some(json!({"model": "gpt", "max_tokens": 10, "messages": [{"role": "user", "content": "hi"}]})),
            &[("Authorization", &auth)],
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let (status, _) = app
        .call_with(
            "POST",
            "/v1/embeddings",
            Some(json!({"model": "gpt", "input": "hi"})),
            &[("Authorization", &auth)],
        )
        .await;
    assert_eq!(status, 200);
}

#[actix_web::test]
async fn router_settings_have_defaults_and_are_validated() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "gpt", "gpt-4o", None).await;

    let (status, settings) = app.call("GET", "/api/router-settings", None).await;
    assert_eq!(status, 200);
    assert_eq!(settings["num_retries"], 2);
    assert_eq!(settings["allowed_fails"], 3);
    assert_eq!(settings["cooldown_time"], 5);
    assert_eq!(settings["fallbacks"], json!([]));

    let (status, body) = app
        .call(
            "PUT",
            "/api/router-settings",
            Some(json!({"fallbacks": [{"gpt": ["nope"]}]})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "fallbacks");
    let (status, body) = app
        .call(
            "PUT",
            "/api/router-settings",
            Some(json!({"num_retries": -1})),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    let (status, saved) = app
        .call(
            "PUT",
            "/api/router-settings",
            Some(json!({"num_retries": 1, "fallbacks": [{"*": ["gpt"]}]})),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(saved["num_retries"], 1);
    assert_eq!(
        saved["cooldown_time"], 5,
        "what is left out keeps its default"
    );

    app.user("bob@example.com", "member").await;
    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "bob@example.com", "password": crate::common::PASSWORD})),
    )
    .await;
    assert_eq!(app.call("GET", "/api/router-settings", None).await.0, 403);
}

#[actix_web::test]
async fn a_lone_deployment_is_never_cooled_down() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "gpt", "rate-limited", None).await;
    router(&mut app, json!({"num_retries": 0})).await;
    let key = setup(&mut app).await;
    for _ in 0..5 {
        assert_eq!(chat(&mut app, &key, "gpt").await.0, 429);
    }
    assert_eq!(
        upstream.count(),
        5,
        "with nowhere else to go, every request is tried"
    );
}

#[actix_web::test]
async fn a_deleted_fallback_is_skipped() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    deploy(&mut app, &upstream, "primary", "fail-500", None).await;
    let backup = deploy(&mut app, &upstream, "backup", "gpt-4o", None).await;
    router(
        &mut app,
        json!({"num_retries": 0, "fallbacks": [{"primary": ["backup"]}]}),
    )
    .await;
    assert_eq!(
        app.call("DELETE", &format!("/api/models/{backup}"), None)
            .await
            .0,
        204
    );
    let key = setup(&mut app).await;
    assert_eq!(chat(&mut app, &key, "primary").await.0, 500);
}

#[actix_web::test]
async fn access_is_by_model_name_whichever_deployment_was_ticked() {
    let (a, b) = (Upstream::start().await, Upstream::start().await);
    let mut app = test_app().await;
    app.admin().await;
    let first = deploy(&mut app, &a, "gpt", "gpt-4o", Some(0)).await;
    let second = deploy(&mut app, &b, "gpt", "gpt-4o", Some(1)).await;
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "T", "models": [first]})),
        )
        .await;
    let (status, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "k", "models": [second]})),
        )
        .await;
    assert_eq!(status, 201, "the same name is allowed: {key}");
    let (status, _, _) = chat(&mut app, key["key"].as_str().unwrap(), "gpt").await;
    assert_eq!(status, 200);
    assert_eq!(b.count(), 1, "any deployment of an allowed name may answer");
}
