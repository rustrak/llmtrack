//! What keys do: personal keys, block and regenerate,
//! budgets that reset, rate limits, explicit "all models", /key/info.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp, PASSWORD};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

async fn model<S>(app: &mut TestApp<S>, upstream: &Upstream, name: &str) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, body) = app
        .call(
            "POST",
            "/api/models",
            Some(
                json!({"name": name, "provider": "openai_compatible", "upstream_model": "gpt-4o",
                        "api_base": upstream.base, "pricing": {"input": 2.5, "output": 10.0}}),
            ),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    body["id"].as_i64().unwrap()
}

async fn team<S>(app: &mut TestApp<S>, body: Value) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, team) = app.call("POST", "/api/teams", Some(body)).await;
    assert_eq!(status, 201, "{team}");
    team["id"].as_i64().unwrap()
}

async fn key<S>(app: &mut TestApp<S>, body: Value) -> Value
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, key) = app.call("POST", "/api/keys", Some(body)).await;
    assert_eq!(status, 201, "{key}");
    key
}

async fn chat<S>(app: &mut TestApp<S>, raw: &Value, model: &str) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {}", raw.as_str().unwrap());
    app.call_with(
        "POST",
        "/v1/chat/completions",
        Some(json!({"model": model, "messages": [{"role": "user", "content": "hi"}]})),
        &[("Authorization", &auth)],
    )
    .await
}

async fn relogin<S>(app: &mut TestApp<S>, email: &str)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.logout_locally();
    let (status, _) = app
        .call(
            "POST",
            "/auth/login",
            Some(json!({"email": email, "password": PASSWORD})),
        )
        .await;
    assert_eq!(status, 200);
}

#[actix_web::test]
async fn a_personal_key_belongs_to_its_user_and_bills_to_them() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let bob = app.user("bob@example.com", "member").await;
    app.user("carol@example.com", "member").await;

    relogin(&mut app, "bob@example.com").await;
    let mine = key(&mut app, json!({"name": "laptop"})).await;
    assert_eq!(mine["team_id"], Value::Null);
    assert_eq!(mine["user_id"], bob);
    assert_eq!(mine["owner_email"], "bob@example.com");
    assert_eq!(
        chat(&mut app, &mine["key"], "gpt").await.0,
        200,
        "a personal key may call every model"
    );

    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(logs["data"][0]["user_id"], bob);
    assert_eq!(logs["data"][0]["team_id"], Value::Null);
    let (_, usage) = app.call("GET", "/api/usage", None).await;
    assert_eq!(
        usage["by_team"][0]["team_id"],
        Value::Null,
        "personal spend is its own bucket"
    );
    assert!(usage["totals"]["cost_usd"].as_f64().unwrap() > 0.0);

    relogin(&mut app, "carol@example.com").await;
    let (_, keys) = app.call("GET", "/api/keys", None).await;
    assert!(
        keys["data"].as_array().unwrap().is_empty(),
        "nobody else sees it"
    );
    let (_, usage) = app.call("GET", "/api/usage", None).await;
    assert_eq!(usage["totals"]["requests"], 0);
    relogin(&mut app, "admin@example.com").await;
    let (_, keys) = app.call("GET", "/api/keys", None).await;
    assert_eq!(
        keys["data"].as_array().unwrap().len(),
        1,
        "admins see every key"
    );
}

#[actix_web::test]
async fn a_blocked_key_is_refused_until_unblocked() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let t = team(&mut app, json!({"name": "Acme", "all_models": true})).await;
    let k = key(&mut app, json!({"team_id": t, "name": "app"})).await;

    assert_eq!(
        app.call("POST", &format!("/api/keys/{}/block", k["id"]), None)
            .await
            .0,
        200
    );
    let (status, body) = chat(&mut app, &k["key"], "gpt").await;
    assert_eq!(status, 401);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("blocked"));
    let (_, keys) = app.call("GET", "/api/keys", None).await;
    assert_eq!(keys["data"][0]["blocked"], true, "blocked keys stay listed");

    assert_eq!(
        app.call("POST", &format!("/api/keys/{}/unblock", k["id"]), None)
            .await
            .0,
        200
    );
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
}

#[actix_web::test]
async fn regenerating_swaps_the_secret_and_keeps_the_key() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let t = team(&mut app, json!({"name": "Acme", "all_models": true})).await;
    let k = key(
        &mut app,
        json!({"team_id": t, "name": "app", "max_budget_usd": 5}),
    )
    .await;
    chat(&mut app, &k["key"], "gpt").await;
    app.state.gateway.flush().await;

    let (status, fresh) = app
        .call("POST", &format!("/api/keys/{}/regenerate", k["id"]), None)
        .await;
    assert_eq!(status, 200, "{fresh}");
    assert_eq!(fresh["id"], k["id"]);
    assert_ne!(fresh["key"], k["key"]);
    assert!(
        fresh["spend_usd"].as_f64().unwrap() > 0.0,
        "spend carries over"
    );
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 401);
    assert_eq!(chat(&mut app, &fresh["key"], "gpt").await.0, 200);
}

#[actix_web::test]
async fn a_budget_with_a_period_resets_when_the_period_ends() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let t = team(&mut app, json!({"name": "Acme", "all_models": true})).await;
    // One request costs $0.000225; $0.0001 a day allows one a day.
    let k = key(
        &mut app,
        json!({"team_id": t, "name": "app", "max_budget_usd": 0.0001, "budget_duration": "1d"}),
    )
    .await;
    assert_eq!(k["budget_duration"], "1d");
    assert!(k["budget_reset_at"].is_string());

    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 429);

    // The day is over.
    app.state.gateway.flush().await;
    sqlx::query("UPDATE api_keys SET budget_reset_at = $1 WHERE id = $2")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .bind(k["id"].as_i64().unwrap())
        .execute(app.pool())
        .await
        .unwrap();
    app.state.gateway.invalidate();
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
    app.state.gateway.flush().await;

    let (_, detail) = app
        .call("GET", &format!("/api/keys/{}", k["id"]), None)
        .await;
    assert!(
        (detail["spend_usd"].as_f64().unwrap() - 0.000225).abs() < 1e-9,
        "{detail}"
    );
    let next =
        chrono::DateTime::parse_from_rfc3339(detail["budget_reset_at"].as_str().unwrap()).unwrap();
    assert!(next > chrono::Utc::now());
}

#[actix_web::test]
async fn team_budgets_reset_too() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let t = team(&mut app, json!({"name": "Acme", "all_models": true, "max_budget_usd": 0.0001, "budget_duration": "30d"})).await;
    let k = key(&mut app, json!({"team_id": t, "name": "app"})).await;
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 429);

    app.state.gateway.flush().await;
    sqlx::query("UPDATE teams SET budget_reset_at = $1 WHERE id = $2")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .bind(t)
        .execute(app.pool())
        .await
        .unwrap();
    app.state.gateway.invalidate();
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
}

#[actix_web::test]
async fn bad_budget_periods_are_refused() {
    let mut app = test_app().await;
    app.admin().await;
    let t = team(&mut app, json!({"name": "Acme"})).await;
    let (status, body) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": t, "name": "k", "budget_duration": "fortnight"})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "budget_duration");
}

#[actix_web::test]
async fn requests_per_minute_are_limited_per_key() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let t = team(&mut app, json!({"name": "Acme", "all_models": true})).await;
    let k = key(
        &mut app,
        json!({"team_id": t, "name": "app", "rpm_limit": 2}),
    )
    .await;

    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
    let (status, body) = chat(&mut app, &k["key"], "gpt").await;
    assert_eq!(status, 429);
    assert_eq!(body["error"]["type"], "rate_limit_exceeded");
    assert_eq!(upstream.count(), 2);
}

#[actix_web::test]
async fn tokens_per_minute_are_limited_per_team() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    // Each request uses 30 tokens.
    let t = team(
        &mut app,
        json!({"name": "Acme", "all_models": true, "tpm_limit": 25}),
    )
    .await;
    let k = key(&mut app, json!({"team_id": t, "name": "app"})).await;

    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 429);
}

#[actix_web::test]
async fn all_models_is_an_explicit_choice() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let none = team(&mut app, json!({"name": "Nothing"})).await;
    let all = team(&mut app, json!({"name": "Everything", "all_models": true})).await;

    let (_, teams) = app.call("GET", &format!("/api/teams/{none}"), None).await;
    assert_eq!(teams["all_models"], false);
    let k = key(&mut app, json!({"team_id": none, "name": "k"})).await;
    assert_eq!(
        chat(&mut app, &k["key"], "gpt").await.0,
        403,
        "nothing ticked means nothing"
    );
    let k = key(&mut app, json!({"team_id": all, "name": "k"})).await;
    assert_eq!(chat(&mut app, &k["key"], "gpt").await.0, 200);
}

#[actix_web::test]
async fn a_key_reads_its_own_spend_and_limits() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let t = team(&mut app, json!({"name": "Acme", "all_models": true})).await;
    let k = key(
        &mut app,
        json!({"team_id": t, "name": "app", "max_budget_usd": 10, "rpm_limit": 60}),
    )
    .await;
    chat(&mut app, &k["key"], "gpt").await;

    let auth = format!("Bearer {}", k["key"].as_str().unwrap());
    let (status, info) = app
        .call_with("GET", "/v1/key/info", None, &[("Authorization", &auth)])
        .await;
    assert_eq!(status, 200, "{info}");
    assert_eq!(info["name"], "app");
    assert_eq!(info["team_name"], "Acme");
    assert_eq!(info["max_budget_usd"], 10.0);
    assert_eq!(info["rpm_limit"], 60);
    assert!(
        info["spend_usd"].as_f64().unwrap() > 0.0,
        "live spend, not the flushed one"
    );
    assert!(info.get("key").is_none());
}

#[actix_web::test]
async fn a_key_has_a_detail_view_and_its_expiry_can_change() {
    let mut app = test_app().await;
    app.admin().await;
    let t = team(&mut app, json!({"name": "Acme"})).await;
    let k = key(&mut app, json!({"team_id": t, "name": "app"})).await;

    let (status, detail) = app
        .call("GET", &format!("/api/keys/{}", k["id"]), None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(detail["name"], "app");
    let (_, updated) = app
        .call(
            "PATCH",
            &format!("/api/keys/{}", k["id"]),
            Some(json!({"expires_at": "2099-01-01T00:00:00Z", "rpm_limit": 10, "budget_duration": "7d"})),
        )
        .await;
    assert!(updated["expires_at"].as_str().unwrap().starts_with("2099"));
    assert_eq!(updated["rpm_limit"], 10);
    assert_eq!(updated["budget_duration"], "7d");
}

#[actix_web::test]
async fn users_show_what_their_keys_spent() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let mine = key(&mut app, json!({"name": "mine"})).await;
    key(&mut app, json!({"name": "spare"})).await;
    chat(&mut app, &mine["key"], "gpt").await;
    app.state.gateway.flush().await;

    let (_, users) = app.call("GET", "/api/users", None).await;
    assert_eq!(users["data"][0]["email"], "admin@example.com");
    assert_eq!(users["data"][0]["key_count"], 2);
    assert!(
        (users["data"][0]["spend_usd"].as_f64().unwrap() - 0.000225).abs() < 1e-9,
        "{users}"
    );
}

#[actix_web::test]
async fn logs_filter_by_time() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    model(&mut app, &upstream, "gpt").await;
    let k = key(&mut app, json!({"name": "mine"})).await;
    chat(&mut app, &k["key"], "gpt").await;
    app.state.gateway.flush().await;

    let hour = |h: i64| {
        (chrono::Utc::now() + chrono::Duration::hours(h))
            .to_rfc3339()
            .replace('+', "%2B")
    };
    let (_, logs) = app
        .call("GET", &format!("/api/logs?from={}", hour(-1)), None)
        .await;
    assert_eq!(logs["data"].as_array().unwrap().len(), 1, "{logs}");
    let (_, logs) = app
        .call("GET", &format!("/api/logs?from={}", hour(1)), None)
        .await;
    assert!(logs["data"].as_array().unwrap().is_empty());
    let (_, logs) = app
        .call("GET", &format!("/api/logs?to={}", hour(-1)), None)
        .await;
    assert!(logs["data"].as_array().unwrap().is_empty());
}
