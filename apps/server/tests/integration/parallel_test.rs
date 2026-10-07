//! `max_parallel_requests`: how many requests a key or a team may
//! have in flight at once.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::test;
use serde_json::{json, Value};

async fn setup<S>(app: &mut TestApp<S>, upstream: &Upstream, team: Value, key: Value) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    app.admin().await;
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "slow", "provider": "openai_compatible",
            "upstream_model": "slow", "api_base": upstream.base})),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    let (_, team) = app.call("POST", "/api/teams", Some(team)).await;
    let mut key = key;
    key["team_id"] = team["id"].clone();
    let (status, key) = app.call("POST", "/api/keys", Some(key)).await;
    assert_eq!(status, 201, "{key}");
    key["key"].as_str().unwrap().to_string()
}

fn chat(key: &str, stream: bool) -> Request {
    test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header(("Authorization", format!("Bearer {key}")))
        .set_json(json!({"model": "slow", "stream": stream,
                         "messages": [{"role": "user", "content": "hi"}]}))
        .to_request()
}

/// Sends `requests` at once and returns their statuses, sorted.
async fn at_once<S>(app: &TestApp<S>, requests: Vec<Request>) -> Vec<u16>
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let calls = requests.into_iter().map(|r| async {
        let response = test::call_service(&app.service, r).await;
        let status = response.status().as_u16();
        // A stream holds its slot until its body is read to the end.
        test::read_body(response).await;
        status
    });
    let mut statuses = futures_util::future::join_all(calls).await;
    statuses.sort_unstable();
    statuses
}

#[actix_web::test]
async fn a_key_may_have_only_so_many_requests_in_flight() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        json!({"name": "T", "all_models": true}),
        json!({"name": "k", "max_parallel_requests": 2}),
    )
    .await;

    assert_eq!(
        at_once(
            &app,
            vec![chat(&key, false), chat(&key, false), chat(&key, false)]
        )
        .await,
        [200, 200, 429]
    );
    assert_eq!(upstream.count(), 2, "the third never reached the provider");
    // Finished requests give their slot back.
    assert_eq!(
        at_once(&app, vec![chat(&key, false), chat(&key, true)]).await,
        [200, 200]
    );

    let (status, body) = app
        .call_with(
            "POST",
            "/v1/chat/completions",
            Some(json!({"model": "slow", "messages": [{"role": "user", "content": "hi"}]})),
            &[("Authorization", &format!("Bearer {key}"))],
        )
        .await;
    assert_eq!(status, 200, "{body}");
}

#[actix_web::test]
async fn a_team_limit_is_shared_by_its_keys() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let first = setup(
        &mut app,
        &upstream,
        json!({"name": "T", "all_models": true, "max_parallel_requests": 1}),
        json!({"name": "a"}),
    )
    .await;
    let (_, team) = app.call("GET", "/api/teams", None).await;
    let (_, second) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["data"][0]["id"], "name": "b"})),
        )
        .await;
    let second = second["key"].as_str().unwrap().to_string();

    assert_eq!(
        at_once(&app, vec![chat(&first, false), chat(&second, false)]).await,
        [200, 429]
    );
}

#[actix_web::test]
async fn a_429_for_parallel_requests_says_so() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        json!({"name": "T", "all_models": true}),
        json!({"name": "k", "max_parallel_requests": 1}),
    )
    .await;
    let calls = [chat(&key, false), chat(&key, false)].map(|r| async {
        let response = test::call_service(&app.service, r).await;
        test::read_body(response).await
    });
    let bodies = futures_util::future::join_all(calls).await;
    let refused = bodies.iter().find_map(|b| {
        let v: Value = serde_json::from_slice(b).ok()?;
        v["error"]["message"].as_str().map(String::from)
    });
    assert!(refused.unwrap().contains("parallel"), "{bodies:?}");
}

#[actix_web::test]
async fn the_limit_is_shown_and_validated() {
    let mut app = test_app().await;
    app.admin().await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"name": "k", "max_parallel_requests": 3})),
        )
        .await;
    assert_eq!(key["max_parallel_requests"], 3);
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "T", "max_parallel_requests": 5})),
        )
        .await;
    assert_eq!(team["max_parallel_requests"], 5);
    let (status, body) = app
        .call(
            "PATCH",
            &format!("/api/keys/{}", key["id"]),
            Some(json!({"max_parallel_requests": 0})),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["fields"][0]["field"], "max_parallel_requests");
    let (_, cleared) = app
        .call(
            "PATCH",
            &format!("/api/keys/{}", key["id"]),
            Some(json!({"max_parallel_requests": null})),
        )
        .await;
    assert_eq!(cleared["max_parallel_requests"], Value::Null);
}
