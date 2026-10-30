//! The MCP endpoint at `/mcp`: the switch that opens it, who may call it,
//! and the tools that read and search what went through the gateway.

use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

use crate::common::upstream::Upstream;
use crate::common::{rpc, test_app, tool_error, tool_output, TestApp, MCP_URL, PUBLIC_URL};

/// Public URL set and `/mcp` on, by the signed-in admin.
async fn open<S>(app: &mut TestApp<S>)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.call(
        "PATCH",
        "/api/settings",
        Some(json!({ "public_url": PUBLIC_URL })),
    )
    .await;
    let (status, body) = app
        .call("PUT", "/api/settings/mcp", Some(json!({ "enabled": true })))
        .await;
    assert_eq!(status, 200, "{body}");
}

/// A model on the fake provider and a key of `team_id` that keeps bodies.
async fn key_keeping_bodies<S>(app: &mut TestApp<S>, upstream: &Upstream, team_id: i64) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, body) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "gpt", "provider": "openai_compatible",
                "upstream_model": "gpt", "api_base": upstream.base,
                "pricing": {"input": 2.5, "output": 10.0}})),
        )
        .await;
    assert!(status == 201 || status == 409, "{body}");
    let (status, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"name": format!("bodies-{team_id}"), "team_id": team_id, "log_bodies": true})),
        )
        .await;
    assert_eq!(status, 201, "{key}");
    key["key"].as_str().unwrap().into()
}

async fn say<S>(app: &mut TestApp<S>, key: &str, text: &str)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    let (status, body) = app
        .call_text(
            "/v1/chat/completions",
            json!({"model": "gpt", "messages": [{"role": "user", "content": text}]}),
            &[("Authorization", auth.as_str())],
        )
        .await;
    assert_eq!(status, 200, "{body}");
}

async fn team<S>(app: &mut TestApp<S>, name: &str) -> i64
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (status, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": name, "all_models": true})),
        )
        .await;
    assert_eq!(status, 201, "{team}");
    team["id"].as_i64().unwrap()
}

#[actix_web::test]
async fn mcp_is_off_until_an_admin_turns_it_on() {
    let mut app = test_app().await;
    let admin = app.admin().await;
    let token = app.mcp_token(admin).await;
    app.call(
        "PATCH",
        "/api/settings",
        Some(json!({ "public_url": PUBLIC_URL })),
    )
    .await;

    let (status, _) = app.call_tool(Some(&token), "get_me", json!({})).await;
    assert_eq!(status, 404, "a fresh install does not answer on /mcp");

    app.logout_locally();
    app.login_as("bob@example.com", "member").await;
    let (status, _) = app
        .call("PUT", "/api/settings/mcp", Some(json!({ "enabled": true })))
        .await;
    assert_eq!(status, 403, "only an admin turns it on");
    let (status, settings) = app.call("GET", "/api/settings/mcp", None).await;
    assert_eq!(status, 200);
    assert_eq!(settings, json!({ "enabled": false, "url": MCP_URL }));

    app.logout_locally();
    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "admin@example.com", "password": crate::common::PASSWORD})),
    )
    .await;
    open(&mut app).await;
    let (status, _) = app.call_tool(Some(&token), "get_me", json!({})).await;
    assert_eq!(status, 200);

    app.call(
        "PUT",
        "/api/settings/mcp",
        Some(json!({ "enabled": false })),
    )
    .await;
    let (status, _) = app.call_tool(Some(&token), "get_me", json!({})).await;
    assert_eq!(status, 404, "turning it off closes it again");
}

#[actix_web::test]
async fn mcp_takes_only_an_oauth_token() {
    let mut app = test_app().await;
    app.admin().await;
    open(&mut app).await;

    let (status, headers, _) = app
        .call_full("POST", "/mcp", Some(rpc("tools/list", json!({}))), &[])
        .await;
    assert_eq!(status, 401);
    assert_eq!(
        headers.get("www-authenticate").unwrap().to_str().unwrap(),
        format!(
            "Bearer resource_metadata=\"{PUBLIC_URL}/.well-known/oauth-protected-resource/mcp\""
        )
    );
    // The dashboard cookie is not a credential here.
    let (status, _) = app.call_tool(None, "get_me", json!({})).await;
    assert_eq!(status, 401);
    let (status, _) = app
        .call_tool(Some("lto_made_up"), "get_me", json!({}))
        .await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn tools_act_with_the_callers_permissions() {
    let mut app = test_app().await;
    app.admin().await;
    open(&mut app).await;
    let mine = team(&mut app, "Mine").await;
    team(&mut app, "Not mine").await;
    let bob = app.user("bob@example.com", "member").await;
    let (status, _) = app
        .call(
            "POST",
            &format!("/api/teams/{mine}/members"),
            Some(json!({"email": "bob@example.com"})),
        )
        .await;
    assert_eq!(status, 201);
    let token = app.mcp_token(bob).await;

    let (_, body) = app.call_tool(Some(&token), "list_teams", json!({})).await;
    let names: Vec<Value> = tool_output(&body)["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].clone())
        .collect();
    assert_eq!(names, [json!("Mine")]);

    let (status, body) = app
        .call_tool(Some(&token), "create_team", json!({"name": "Sneaky"}))
        .await;
    assert_eq!(
        status, 200,
        "a refused tool is a tool error, not an HTTP one"
    );
    assert!(tool_error(&body).unwrap().contains("admin only"), "{body}");
}

#[actix_web::test]
async fn admins_create_through_tools() {
    let mut app = test_app().await;
    let admin = app.admin().await;
    open(&mut app).await;
    let token = app.mcp_token(admin).await;

    let (_, body) = app
        .call_tool(
            Some(&token),
            "create_team",
            json!({"name": "Support", "all_models": true}),
        )
        .await;
    let team = tool_output(&body);
    assert_eq!(team["name"], "Support", "{body}");
    let (_, body) = app
        .call_tool(
            Some(&token),
            "create_key",
            json!({"name": "bot", "team_id": team["id"], "log_bodies": true}),
        )
        .await;
    let key = tool_output(&body);
    assert!(key["key"].as_str().unwrap().starts_with("sk-"), "{body}");
}

#[actix_web::test]
async fn conversations_are_found_by_their_words_and_only_by_who_may_see_them() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let admin = app.admin().await;
    open(&mut app).await;
    let support = team(&mut app, "Support").await;
    let sales = team(&mut app, "Sales").await;
    let support_key = key_keeping_bodies(&mut app, &upstream, support).await;
    let sales_key = key_keeping_bodies(&mut app, &upstream, sales).await;
    say(
        &mut app,
        &support_key,
        "Where is the REFUND for order 1234?",
    )
    .await;
    say(&mut app, &support_key, "Change my shipping address").await;
    say(&mut app, &sales_key, "Quote a refund policy for enterprise").await;
    app.state.gateway.flush().await;

    let token = app.mcp_token(admin).await;
    let (_, body) = app
        .call_tool(
            Some(&token),
            "list_requests",
            json!({"text": "refund order"}),
        )
        .await;
    let page = tool_output(&body);
    assert_eq!(page["total"], 1, "every word must appear: {page}");
    let hit = &page["data"][0];
    assert!(
        hit["snippet"]
            .as_str()
            .unwrap()
            .contains("REFUND for order 1234"),
        "{hit}"
    );

    let (_, body) = app
        .call_tool(
            Some(&token),
            "list_requests",
            json!({"text": "refund", "team_id": [sales]}),
        )
        .await;
    assert_eq!(
        tool_output(&body)["total"],
        1,
        "filters narrow a text search"
    );

    // The same filter on the REST API.
    let (_, logs) = app.call("GET", "/api/logs?text=refund", None).await;
    assert_eq!(logs["total"], 2);

    let (_, body) = app
        .call_tool(
            Some(&token),
            "get_request",
            json!({"request_id": hit["request_id"]}),
        )
        .await;
    let request = tool_output(&body);
    assert_eq!(
        request["body"]["request"]["messages"][0]["content"],
        "Where is the REFUND for order 1234?"
    );
    assert_eq!(request["log"]["model_name"], "gpt");

    let (_, body) = app
        .call_tool(Some(&token), "search", json!({"query": "support"}))
        .await;
    let found = tool_output(&body);
    assert_eq!(found["teams"]["total"], 1, "{found}");
    assert_eq!(found["teams"]["data"][0]["name"], "Support");
    assert!(found["users"].is_object(), "admins also search users");

    // A member of Support finds Support's conversations only.
    let bob = app.user("bob@example.com", "member").await;
    app.call(
        "POST",
        &format!("/api/teams/{support}/members"),
        Some(json!({"email": "bob@example.com"})),
    )
    .await;
    let token = app.mcp_token(bob).await;
    let (_, body) = app
        .call_tool(Some(&token), "search", json!({"query": "refund"}))
        .await;
    let found = tool_output(&body);
    assert_eq!(found["requests"]["total"], 1, "{found}");
    assert!(found.get("users").is_none());
}
