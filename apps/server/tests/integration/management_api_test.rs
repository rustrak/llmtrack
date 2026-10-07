//! The management API (`/api`) with a bearer key instead of a session:
//! the master key acts as an admin, a personal key as its owner.

use crate::common::upstream::Upstream;
use crate::common::{test_app, test_app_with, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

const MASTER: &str = "sk-master-0123456789abcdef";

async fn bearer<S>(
    app: &mut TestApp<S>,
    key: &str,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    app.call_with(method, uri, body, &[("Authorization", &auth)])
        .await
}

#[actix_web::test]
async fn the_master_key_manages_everything_as_the_first_admin() {
    let upstream = Upstream::start().await;
    let mut app = test_app_with(Some(MASTER)).await;
    let admin = app.user("root@example.com", "admin").await;

    let (status, model) = bearer(
        &mut app,
        MASTER,
        "POST",
        "/api/models",
        Some(json!({
        "name": "gpt", "provider": "openai_compatible", "upstream_model": "gpt-4o",
        "api_base": upstream.base})),
    )
    .await;
    assert_eq!(status, 201, "{model}");
    let (status, team) = bearer(
        &mut app,
        MASTER,
        "POST",
        "/api/teams",
        Some(json!({"name": "Acme", "all_models": true})),
    )
    .await;
    assert_eq!(status, 201, "{team}");
    let (status, key) = bearer(
        &mut app,
        MASTER,
        "POST",
        "/api/keys",
        Some(json!({"team_id": team["id"], "name": "ci"})),
    )
    .await;
    assert_eq!(status, 201, "{key}");
    assert_eq!(key["created_by"], admin);

    let auth = format!("Bearer {}", key["key"].as_str().unwrap());
    let (status, _) = app
        .call_with(
            "POST",
            "/v1/chat/completions",
            Some(json!({"model": "gpt", "messages": [{"role": "user", "content": "hi"}]})),
            &[("Authorization", &auth)],
        )
        .await;
    assert_eq!(status, 200, "a key made through the API works");
}

#[actix_web::test]
async fn a_wrong_or_missing_master_key_is_401() {
    let mut app = test_app_with(Some(MASTER)).await;
    app.user("root@example.com", "admin").await;
    assert_eq!(
        bearer(&mut app, "sk-master-wrong", "GET", "/api/teams", None)
            .await
            .0,
        401
    );
    let mut app = test_app().await;
    app.user("root@example.com", "admin").await;
    assert_eq!(
        bearer(&mut app, MASTER, "GET", "/api/teams", None).await.0,
        401,
        "no master key set: nothing matches"
    );
}

#[actix_web::test]
async fn a_personal_key_acts_as_its_owner() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, admin_key) = app
        .call("POST", "/api/keys", Some(json!({"name": "automation"})))
        .await;
    let admin_key = admin_key["key"].as_str().unwrap().to_string();
    app.login_as("bob@example.com", "member").await;
    let (_, bob_key) = app
        .call("POST", "/api/keys", Some(json!({"name": "bob"})))
        .await;
    let bob_key = bob_key["key"].as_str().unwrap().to_string();
    app.logout_locally();

    let (status, team) = bearer(
        &mut app,
        &admin_key,
        "POST",
        "/api/teams",
        Some(json!({"name": "Acme"})),
    )
    .await;
    assert_eq!(status, 201, "an admin's key manages like an admin: {team}");
    assert_eq!(
        bearer(
            &mut app,
            &bob_key,
            "POST",
            "/api/teams",
            Some(json!({"name": "Bob's"}))
        )
        .await
        .0,
        403
    );
    let (status, keys) = bearer(&mut app, &bob_key, "GET", "/api/keys", None).await;
    assert_eq!(status, 200);
    assert_eq!(
        keys["data"].as_array().unwrap().len(),
        1,
        "a member's key sees what the member sees"
    );
}

#[actix_web::test]
async fn team_blocked_and_revoked_keys_cannot_manage() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, team) = app
        .call("POST", "/api/teams", Some(json!({"name": "Acme"})))
        .await;
    let (_, team_key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "app"})),
        )
        .await;
    let (_, mine) = app
        .call("POST", "/api/keys", Some(json!({"name": "mine"})))
        .await;
    app.call("POST", &format!("/api/keys/{}/block", mine["id"]), None)
        .await;
    app.logout_locally();

    let (status, body) = bearer(
        &mut app,
        team_key["key"].as_str().unwrap(),
        "GET",
        "/api/teams",
        None,
    )
    .await;
    assert_eq!(status, 403, "{body}");
    assert_eq!(
        bearer(
            &mut app,
            mine["key"].as_str().unwrap(),
            "GET",
            "/api/teams",
            None
        )
        .await
        .0,
        401
    );
    assert_eq!(
        bearer(&mut app, "sk-nonsense", "GET", "/api/teams", None)
            .await
            .0,
        401
    );
}
