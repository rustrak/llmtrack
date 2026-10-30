//! OAuth 2.1 for `/mcp`: llmtrack as its own authorization server, so an
//! assistant connects with the user's consent. The client's metadata document
//! is served by `FakeClients`.

use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::test;
use base64::Engine;
use serde_json::{json, Value};
use sha2::Digest;

use crate::common::{test_app, TestApp, CLIENT_ID, MCP_URL, PUBLIC_URL, REDIRECT, SCRIPT_CLIENT};

const VERIFIER: &str = "a-verifier-that-is-long-enough-to-be-valid-0123456789";

fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier))
}

fn authorize_query(client_id: &str, redirect: &str) -> String {
    let query = [
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("code_challenge", &challenge(VERIFIER)),
        ("code_challenge_method", "S256"),
        ("state", "xyz"),
        ("resource", MCP_URL),
    ];
    let mut url = reqwest::Url::parse("http://x/api/oauth/authorize").unwrap();
    url.query_pairs_mut().extend_pairs(query);
    format!("/api/oauth/authorize?{}", url.query().unwrap())
}

/// An admin signed in, with the public URL set and `/mcp` on.
async fn ready<S>(app: &mut TestApp<S>) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let admin = app.admin().await;
    let (status, _) = app
        .call(
            "PATCH",
            "/api/settings",
            Some(json!({ "public_url": PUBLIC_URL })),
        )
        .await;
    assert_eq!(status, 200);
    let (status, _) = app
        .call("PUT", "/api/settings/mcp", Some(json!({ "enabled": true })))
        .await;
    assert_eq!(status, 200);
    admin
}

/// Approves the request and returns the code the browser is sent back with.
async fn approve<S>(app: &mut TestApp<S>) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let request = json!({
        "response_type": "code",
        "client_id": CLIENT_ID,
        "redirect_uri": REDIRECT,
        "code_challenge": challenge(VERIFIER),
        "code_challenge_method": "S256",
        "state": "xyz",
        "approve": true,
    });
    let (status, body) = app
        .call("POST", "/api/oauth/authorize", Some(request))
        .await;
    assert_eq!(status, 200, "{body}");
    let redirect = reqwest::Url::parse(body["redirect_to"].as_str().unwrap()).unwrap();
    assert!(redirect.as_str().starts_with(REDIRECT));
    let param = |name: &str| {
        redirect
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    assert_eq!(param("state").as_deref(), Some("xyz"));
    assert_eq!(param("iss").as_deref(), Some(PUBLIC_URL));
    param("code").unwrap()
}

async fn token<S>(app: &TestApp<S>, form: &[(&str, &str)]) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let req = test::TestRequest::post()
        .uri("/api/oauth/token")
        .set_form(form)
        .to_request();
    let response = test::call_service(&app.service, req).await;
    let status = response.status().as_u16();
    (status, test::read_body_json(response).await)
}

async fn exchange<S>(app: &TestApp<S>, code: &str, verifier: &str) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    token(
        app,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT),
            ("code_verifier", verifier),
            ("resource", MCP_URL),
        ],
    )
    .await
}

#[actix_web::test]
async fn discovery_needs_the_public_url() {
    let mut app = test_app().await;
    let (status, _) = app
        .call("GET", "/.well-known/oauth-authorization-server", None)
        .await;
    assert_eq!(status, 404);

    ready(&mut app).await;
    let (status, server) = app
        .call("GET", "/.well-known/oauth-authorization-server", None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(server["issuer"], PUBLIC_URL);
    assert_eq!(
        server["authorization_endpoint"],
        format!("{PUBLIC_URL}/oauth/authorize")
    );
    assert_eq!(
        server["token_endpoint"],
        format!("{PUBLIC_URL}/api/oauth/token")
    );
    let (_, resource) = app
        .call("GET", "/.well-known/oauth-protected-resource/mcp", None)
        .await;
    assert_eq!(resource["resource"], MCP_URL);
    assert_eq!(resource["authorization_servers"][0], PUBLIC_URL);
}

#[actix_web::test]
async fn an_approved_assistant_calls_tools_as_its_user() {
    let mut app = test_app().await;
    ready(&mut app).await;

    let (status, info) = app
        .call("GET", &authorize_query(CLIENT_ID, REDIRECT), None)
        .await;
    assert_eq!(status, 200, "{info}");
    assert_eq!(info["client_name"], "Example Assistant");
    assert_eq!(info["redirect_host"], "client.example");

    let code = approve(&mut app).await;
    let (status, pair) = exchange(&app, &code, VERIFIER).await;
    assert_eq!(status, 200, "{pair}");
    let access = pair["access_token"].as_str().unwrap().to_string();
    assert!(access.starts_with("lto_"));

    let (status, body) = app.call_tool(Some(&access), "get_me", json!({})).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        crate::common::tool_output(&body)["user"]["email"],
        "admin@example.com"
    );

    // A code is spent by being presented.
    let (status, error) = exchange(&app, &code, VERIFIER).await;
    assert_eq!(
        (status, error["error"].as_str()),
        (400, Some("invalid_grant"))
    );

    // The token opens /mcp only, never the management API.
    app.logout_locally();
    let auth = format!("Bearer {access}");
    let (status, _) = app
        .call_with("GET", "/auth/me", None, &[("Authorization", auth.as_str())])
        .await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn a_refresh_token_works_once_and_its_reuse_disconnects() {
    let mut app = test_app().await;
    ready(&mut app).await;
    let code = approve(&mut app).await;
    let (_, first) = exchange(&app, &code, VERIFIER).await;
    let refresh = first["refresh_token"].as_str().unwrap();

    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
        ("client_id", CLIENT_ID),
    ];
    let (status, second) = token(&app, &form).await;
    assert_eq!(status, 200, "{second}");
    let access = second["access_token"].as_str().unwrap().to_string();
    let (status, _) = app.call_tool(Some(&access), "get_me", json!({})).await;
    assert_eq!(status, 200);

    // The old refresh token again: someone else holds it, so both lose it.
    let (status, _) = token(&app, &form).await;
    assert_eq!(status, 400);
    let (status, _) = app.call_tool(Some(&access), "get_me", json!({})).await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn untrusted_requests_are_refused_and_never_redirected() {
    let mut app = test_app().await;
    ready(&mut app).await;

    let (status, _) = app
        .call(
            "GET",
            &authorize_query(CLIENT_ID, "https://attacker.example/cb"),
            None,
        )
        .await;
    assert_eq!(status, 400, "a redirect the client did not register");
    let (status, _) = app
        .call(
            "GET",
            &authorize_query(SCRIPT_CLIENT, "javascript:alert(document.domain)//"),
            None,
        )
        .await;
    assert_eq!(status, 400, "a script is never a redirect");
    let (status, _) = app
        .call(
            "GET",
            &authorize_query("https://unknown.example/m.json", REDIRECT),
            None,
        )
        .await;
    assert_eq!(status, 400);

    let code = approve(&mut app).await;
    let (status, error) = exchange(&app, &code, &VERIFIER.replace('a', "b")).await;
    assert_eq!(
        (status, error["error"].as_str()),
        (400, Some("invalid_grant"))
    );

    app.logout_locally();
    let (status, _) = app
        .call("GET", &authorize_query(CLIENT_ID, REDIRECT), None)
        .await;
    assert_eq!(status, 401, "consent needs a signed-in user");
}

#[actix_web::test]
async fn users_see_and_disconnect_their_assistants() {
    let mut app = test_app().await;
    ready(&mut app).await;
    let code = approve(&mut app).await;
    let (_, pair) = exchange(&app, &code, VERIFIER).await;
    let access = pair["access_token"].as_str().unwrap().to_string();

    let (status, grants) = app.call("GET", "/api/oauth/grants", None).await;
    assert_eq!(status, 200);
    assert_eq!(grants[0]["client_name"], "Example Assistant");
    let id = grants[0]["id"].as_i64().unwrap();

    app.logout_locally();
    app.login_as("bob@example.com", "member").await;
    let (status, _) = app
        .call("DELETE", &format!("/api/oauth/grants/{id}"), None)
        .await;
    assert_eq!(status, 404, "only its own user disconnects it");
    let (_, theirs) = app.call("GET", "/api/oauth/grants", None).await;
    assert_eq!(theirs, json!([]));

    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "admin@example.com", "password": crate::common::PASSWORD})),
    )
    .await;
    let (status, _) = app
        .call("DELETE", &format!("/api/oauth/grants/{id}"), None)
        .await;
    assert_eq!(status, 204);
    let (status, _) = app.call_tool(Some(&access), "get_me", json!({})).await;
    assert_eq!(status, 401);
}
