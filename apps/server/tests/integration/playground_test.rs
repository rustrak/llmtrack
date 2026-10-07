//! The dashboard's Playground: a signed-in user chats through one of the
//! keys they manage, without the browser ever holding the key.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp, PASSWORD};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use serde_json::{json, Value};

/// A priced model on the fake upstream and a personal key; the key's id.
async fn setup<S>(app: &mut TestApp<S>, upstream: &Upstream) -> (i64, i64)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.admin().await;
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "gpt", "provider": "openai_compatible",
                "upstream_model": "gpt", "api_base": upstream.base,
                "pricing": {"input": 2.5, "output": 10.0}})),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    let (_, key) = app
        .call("POST", "/api/keys", Some(json!({"name": "mine"})))
        .await;
    (key["id"].as_i64().unwrap(), model["id"].as_i64().unwrap())
}

fn chat(key_id: i64) -> Value {
    json!({"key_id": key_id, "model": "gpt",
           "messages": [{"role": "user", "content": "hi"}]})
}

#[actix_web::test]
async fn a_chat_goes_through_the_chosen_key_and_is_billed_to_it() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (key_id, model_id) = setup(&mut app, &upstream).await;

    let (status, headers, reply) = app
        .call_full("POST", "/api/playground/chat", Some(chat(key_id)), &[])
        .await;
    assert_eq!(status, 200, "{reply}");
    assert!(reply["choices"][0]["message"]["content"].is_string());
    assert_eq!(
        headers.get("x-litellm-model-id").unwrap().to_str().unwrap(),
        model_id.to_string(),
        "the deployment that answered, to price the reply"
    );
    assert!(
        upstream.last().body.get("key_id").is_none(),
        "the key id is ours"
    );

    app.state.gateway.flush().await;
    let (_, logs) = app.call("GET", "/api/logs", None).await;
    assert_eq!(logs["data"][0]["key_id"], key_id);
    assert_eq!(logs["data"][0]["tags"], json!(["playground"]));
}

#[actix_web::test]
async fn a_chat_streams() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (key_id, _) = setup(&mut app, &upstream).await;
    let mut body = chat(key_id);
    body["stream"] = json!(true);
    body["stream_options"] = json!({"include_usage": true});

    let (status, text) = app.call_text("/api/playground/chat", body, &[]).await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("data: "), "{text}");
    assert!(text.contains("\"usage\""), "usage comes last: {text}");
}

#[actix_web::test]
async fn a_reply_is_priced_by_the_deployment_that_answered() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (_, model_id) = setup(&mut app, &upstream).await;

    let (status, cost) = app
        .call(
            "POST",
            "/api/playground/cost",
            Some(json!({"model_id": model_id,
                        "usage": {"prompt_tokens": 1_000_000, "completion_tokens": 100_000}})),
        )
        .await;
    assert_eq!(status, 200, "{cost}");
    assert_eq!(cost["cost_usd"], 3.5);
}

#[actix_web::test]
async fn only_keys_you_manage_and_only_when_signed_in() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (key_id, _) = setup(&mut app, &upstream).await;

    app.user("bob@example.com", "member").await;
    app.logout_locally();
    let (status, _) = app
        .call("POST", "/api/playground/chat", Some(chat(key_id)))
        .await;
    assert_eq!(status, 401);

    app.call(
        "POST",
        "/auth/login",
        Some(json!({"email": "bob@example.com", "password": PASSWORD})),
    )
    .await;
    let (status, _) = app
        .call("POST", "/api/playground/chat", Some(chat(key_id)))
        .await;
    assert_eq!(status, 404, "someone else's personal key does not exist");
    assert_eq!(upstream.count(), 0);
}

#[actix_web::test]
async fn it_lists_the_models_the_key_may_call() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let (key_id, model_id) = setup(&mut app, &upstream).await;
    app.call(
        "POST",
        "/api/models",
        Some(json!({"name": "other", "provider": "openai_compatible",
            "upstream_model": "other", "api_base": upstream.base})),
    )
    .await;
    app.call(
        "PATCH",
        &format!("/api/keys/{key_id}"),
        Some(json!({"models": [model_id]})),
    )
    .await;

    let (status, models) = app
        .call(
            "GET",
            &format!("/api/playground/models?key_id={key_id}"),
            None,
        )
        .await;
    assert_eq!(status, 200, "{models}");
    assert_eq!(models, json!(["gpt"]));
}
