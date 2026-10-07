//! Spend by end user ("customers") and by tag.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

async fn setup<S>(app: &mut TestApp<S>, upstream: &Upstream) -> String
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
    let (_, key) = app
        .call("POST", "/api/keys", Some(json!({"name": "k"})))
        .await;
    key["key"].as_str().unwrap().to_string()
}

async fn send<S>(
    app: &mut TestApp<S>,
    key: &str,
    path: &str,
    body: Value,
    headers: &[(&str, &str)],
) -> u16
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    let mut all = vec![("Authorization", auth.as_str())];
    all.extend_from_slice(headers);
    let (status, body) = app.call_with("POST", path, Some(body), &all).await;
    assert_eq!(status, 200, "{body}");
    status
}

fn hi(model: &str) -> Value {
    json!({"model": model, "messages": [{"role": "user", "content": "hi"}]})
}

#[actix_web::test]
async fn the_end_user_comes_from_the_body_or_a_header() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(&mut app, &upstream).await;

    let mut body = hi("gpt");
    body["user"] = json!("customer-1");
    send(&mut app, &key, "/v1/chat/completions", body.clone(), &[]).await;
    assert_eq!(
        upstream.last().body["user"],
        "customer-1",
        "`user` is OpenAI's own field: it goes on"
    );
    send(
        &mut app,
        &key,
        "/v1/chat/completions",
        body,
        &[("x-litellm-customer-id", "customer-2")],
    )
    .await;
    let mut messages = json!({"model": "claude", "max_tokens": 5, "messages": [{"role": "user", "content": "hi"}]});
    messages["metadata"] = json!({"user_id": "customer-3"});
    send(&mut app, &key, "/v1/messages", messages, &[]).await;
    send(&mut app, &key, "/v1/chat/completions", hi("gpt"), &[]).await;

    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    let users: Vec<&Value> = logs["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| &l["end_user"])
        .collect();
    assert_eq!(
        users,
        [
            &Value::Null,
            &json!("customer-3"),
            &json!("customer-2"),
            &json!("customer-1")
        ]
    );

    let (_, page) = app.call("GET", "/api/logs?end_user=customer-2", None).await;
    assert_eq!(page["data"].as_array().unwrap().len(), 1);
    let (_, usage) = app.call("GET", "/api/usage", None).await;
    let names: Vec<&str> = usage["by_end_user"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["end_user"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 3, "{usage}");
    assert!(usage["by_end_user"][0]["cost_usd"].as_f64().unwrap() > 0.0);
}

#[actix_web::test]
async fn tags_come_from_metadata_the_body_and_a_header_and_stay_here() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(&mut app, &upstream).await;

    let mut body = hi("gpt");
    body["metadata"] = json!({"tags": ["prod", "search"], "trace": "x"});
    body["tags"] = json!(["batch"]);
    send(
        &mut app,
        &key,
        "/v1/chat/completions",
        body,
        &[("x-litellm-tags", "prod, eu")],
    )
    .await;
    let sent = upstream.last().body;
    assert!(sent.get("tags").is_none(), "{sent}");
    assert_eq!(
        sent["metadata"],
        json!({"trace": "x"}),
        "the rest of the metadata goes on"
    );

    let mut plain = hi("gpt");
    plain["metadata"] = json!({"tags": ["prod"]});
    send(&mut app, &key, "/v1/chat/completions", plain, &[]).await;
    assert!(
        upstream.last().body.get("metadata").is_none(),
        "emptied metadata is dropped"
    );

    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(
        logs["data"][1]["tags"],
        json!(["prod", "eu", "batch", "search"])
    );
    let (_, page) = app.call("GET", "/api/logs?tag=search", None).await;
    assert_eq!(page["data"].as_array().unwrap().len(), 1);

    let (_, usage) = app.call("GET", "/api/usage", None).await;
    let by_tag: Vec<(String, i64)> = usage["by_tag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["tag"].as_str().unwrap().to_string(),
                r["requests"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(by_tag[0], ("prod".to_string(), 2), "{usage}");
    assert_eq!(by_tag.len(), 4);
}

#[actix_web::test]
async fn attribution_is_bounded() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(&mut app, &upstream).await;
    let mut body = hi("gpt");
    body["user"] = json!("u".repeat(1000));
    body["tags"] = json!((0..50).map(|i| format!("t{i}")).collect::<Vec<_>>());
    send(&mut app, &key, "/v1/chat/completions", body, &[]).await;
    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(logs["data"][0]["end_user"].as_str().unwrap().len(), 256);
    assert_eq!(logs["data"][0]["tags"].as_array().unwrap().len(), 20);
}

#[actix_web::test]
async fn members_only_see_their_own_customers_and_tags() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(&mut app, &upstream).await;
    let mut body = hi("gpt");
    body["user"] = json!("admins-customer");
    body["tags"] = json!(["secret"]);
    send(&mut app, &key, "/v1/chat/completions", body, &[]).await;
    app.state.gateway.flush().await;

    app.login_as("bob@example.com", "member").await;
    let (_, usage) = app.call("GET", "/api/usage", None).await;
    assert!(usage["by_end_user"].as_array().unwrap().is_empty());
    assert!(usage["by_tag"].as_array().unwrap().is_empty());
}
