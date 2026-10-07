//! Every endpoint besides chat completions, and the billing details: cache,
//! reasoning, images, characters, seconds.

use crate::common::upstream::Upstream;
use crate::common::{test_app, TestApp};
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::test;
use serde_json::{json, Value};

/// One model on the fake upstream with the given rates, a team and a key.
async fn setup<S>(
    app: &mut TestApp<S>,
    upstream: &Upstream,
    provider: &str,
    name: &str,
    upstream_model: &str,
    pricing: Value,
) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    if app.cookie.is_none() {
        app.admin().await;
    }
    let (status, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": name, "provider": provider, "upstream_model": upstream_model,
                        "api_base": upstream.base, "api_key": "provider-secret", "pricing": pricing})),
        )
        .await;
    assert_eq!(status, 201, "{model}");
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": format!("team-{name}"), "all_models": true})),
        )
        .await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "app"})),
        )
        .await;
    key["key"].as_str().unwrap().to_string()
}

async fn post<S>(app: &mut TestApp<S>, uri: &str, key: &str, body: Value) -> (u16, Value)
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    app.call_with("POST", uri, Some(body), &[("Authorization", &auth)])
        .await
}

async fn stream<S>(app: &mut TestApp<S>, uri: &str, key: &str, body: Value) -> String
where
    S: Service<Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let auth = format!("Bearer {key}");
    let (status, text) = app.call_text(uri, body, &[("Authorization", &auth)]).await;
    assert_eq!(status, 200, "{text}");
    text
}

#[derive(Debug, sqlx::FromRow)]
struct Log {
    endpoint: String,
    prompt_tokens: i64,
    completion_tokens: i64,
    cached_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    cost_nanos: i64,
}

async fn last_log<S>(app: &TestApp<S>) -> Log {
    app.state.gateway.flush().await;
    sqlx::query_as::<_, Log>(
        "SELECT endpoint, prompt_tokens, completion_tokens, cached_tokens, cache_write_tokens,
                reasoning_tokens, cost_nanos
         FROM request_logs ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&app.db.pool)
    .await
    .unwrap()
}

fn chat(model: &str) -> Value {
    json!({"model": model, "messages": [{"role": "user", "content": "Say hello"}]})
}

#[actix_web::test]
async fn cached_and_reasoning_tokens_are_billed_at_their_own_rates() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "m",
        "cached-model",
        json!({"input": 2.0, "output": 10.0, "cache_read": 0.5, "reasoning": 20.0}),
    )
    .await;

    let (status, _) = post(&mut app, "/v1/chat/completions", &key, chat("m")).await;
    assert_eq!(status, 200);

    let log = last_log(&app).await;
    assert_eq!((log.prompt_tokens, log.cached_tokens), (10, 6));
    assert_eq!((log.completion_tokens, log.reasoning_tokens), (20, 5));
    // 4 text in × $2 + 6 cached × $0.50 + 15 text out × $10 + 5 reasoning × $20, per 1M.
    assert_eq!(
        log.cost_nanos,
        4 * 2_000 + 6 * 500 + 15 * 10_000 + 5 * 20_000
    );
}

#[actix_web::test]
async fn anthropic_cache_reads_and_writes_are_billed() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "cached-claude",
        json!({"input": 3.0, "output": 15.0, "cache_read": 0.3, "cache_write": 3.75}),
    )
    .await;

    let (status, body) = post(&mut app, "/v1/chat/completions", &key, chat("claude")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["usage"]["prompt_tokens"], 160);
    assert_eq!(body["usage"]["prompt_tokens_details"]["cached_tokens"], 100);

    let log = last_log(&app).await;
    assert_eq!(
        (log.prompt_tokens, log.cached_tokens, log.cache_write_tokens),
        (160, 100, 50)
    );
    assert_eq!(
        log.cost_nanos,
        10 * 3_000 + 100 * 300 + 50 * 3_750 + 20 * 15_000
    );
}

#[actix_web::test]
async fn a_catalog_priced_model_bills_without_any_rates_typed_in() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    app.admin().await;
    let (_, model) = app
        .call(
            "POST",
            "/api/models",
            Some(json!({"name": "4o", "provider": "openai", "upstream_model": "gpt-4o", "api_base": upstream.base})),
        )
        .await;
    assert_eq!(model["pricing_source"], "catalog");
    let (_, team) = app
        .call(
            "POST",
            "/api/teams",
            Some(json!({"name": "t", "all_models": true})),
        )
        .await;
    let (_, key) = app
        .call(
            "POST",
            "/api/keys",
            Some(json!({"team_id": team["id"], "name": "k"})),
        )
        .await;

    post(
        &mut app,
        "/v1/chat/completions",
        key["key"].as_str().unwrap(),
        chat("4o"),
    )
    .await;
    // gpt-4o in the price list: $2.50 in, $10 out per 1M.
    assert_eq!(last_log(&app).await.cost_nanos, 10 * 2_500 + 20 * 10_000);
}

#[actix_web::test]
async fn legacy_completions_are_proxied_and_streamed() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "instruct",
        "gpt-3.5-turbo-instruct",
        json!({"input": 1.0, "output": 2.0}),
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/completions",
        &key,
        json!({"model": "instruct", "prompt": "Say"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(upstream.last().path, "/v1/completions");
    assert_eq!(upstream.last().body["model"], "gpt-3.5-turbo-instruct");
    assert_eq!(last_log(&app).await.endpoint, "completions");

    let text = stream(
        &mut app,
        "/v1/completions",
        &key,
        json!({"model": "instruct", "prompt": "Say", "stream": true}),
    )
    .await;
    assert!(text.contains("Hello") && !text.contains("\"usage\""));
    assert_eq!(last_log(&app).await.completion_tokens, 20);
}

#[actix_web::test]
async fn the_responses_api_is_billed_from_its_own_usage_shape() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "o",
        "o4-mini",
        json!({"input": 1.0, "output": 4.0}),
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/responses",
        &key,
        json!({"model": "o", "input": "hi"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["object"], "response");
    let log = last_log(&app).await;
    assert_eq!(log.endpoint, "responses");
    assert_eq!(
        (log.prompt_tokens, log.cached_tokens, log.reasoning_tokens),
        (10, 4, 5)
    );

    let text = stream(
        &mut app,
        "/v1/responses",
        &key,
        json!({"model": "o", "input": "hi", "stream": true}),
    )
    .await;
    assert!(text.contains("response.completed"));
    let log = last_log(&app).await;
    assert_eq!((log.prompt_tokens, log.completion_tokens), (10, 20));
}

#[actix_web::test]
async fn images_are_billed_per_image() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "dalle",
        "dall-e-3",
        json!({"per_image": 0.04}),
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/images/generations",
        &key,
        json!({"model": "dalle", "prompt": "a cat", "n": 2}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert_eq!(last_log(&app).await.cost_nanos, 2 * 40_000_000);
}

#[actix_web::test]
async fn speech_returns_audio_and_is_billed_per_character() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "tts",
        "tts-1",
        json!({"per_million_characters": 15.0}),
    )
    .await;

    let auth = format!("Bearer {key}");
    let response = test::call_service(
        &app.service,
        test::TestRequest::post()
            .uri("/v1/audio/speech")
            .insert_header(("Authorization", auth.as_str()))
            .set_json(json!({"model": "tts", "input": "Hello world", "voice": "alloy"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "audio/mpeg"
    );
    assert_eq!(test::read_body(response).await, &b"ID3-fake-audio"[..]);
    assert_eq!(last_log(&app).await.cost_nanos, 11 * 15_000);
}

#[actix_web::test]
async fn moderations_pass_through_free() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "mod",
        "omni-moderation-latest",
        json!({"input": 0.0, "output": 0.0}),
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/moderations",
        &key,
        json!({"model": "mod", "input": "hi"}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["results"][0]["flagged"], false);
    assert_eq!(last_log(&app).await.cost_nanos, 0);
}

#[actix_web::test]
async fn transcriptions_forward_the_upload_with_the_provider_model() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "whisper",
        "whisper-1",
        json!({"per_second": 0.0001}),
    )
    .await;

    let boundary = "XBOUNDARYX";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nwhisper\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\nFAKEAUDIO\r\n\
         --{boundary}--\r\n"
    );
    let auth = format!("Bearer {key}");
    let response = test::call_service(
        &app.service,
        test::TestRequest::post()
            .uri("/v1/audio/transcriptions")
            .insert_header(("Authorization", auth.as_str()))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), 200);
    let sent = upstream.last();
    assert_eq!(sent.path, "/v1/audio/transcriptions");
    let raw = sent.body.as_str().unwrap();
    assert!(raw.contains("whisper-1") && raw.contains("FAKEAUDIO") && raw.contains("a.mp3"));
    // 3 seconds × $0.0001.
    assert_eq!(last_log(&app).await.cost_nanos, 300_000);
}

#[actix_web::test]
async fn anthropic_models_only_take_chat_and_messages() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
        json!({"input": 3.0, "output": 15.0}),
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/embeddings",
        &key,
        json!({"model": "claude", "input": "hi"}),
    )
    .await;
    assert_eq!(status, 400, "{body}");
}

#[actix_web::test]
async fn messages_to_an_anthropic_model_pass_through_natively() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "cached-claude",
        json!({"input": 3.0, "output": 15.0, "cache_read": 0.3}),
    )
    .await;

    let request = json!({"model": "claude", "max_tokens": 100, "messages": [{"role": "user", "content": "hi"}]});
    let (status, body) = app
        .call_with(
            "POST",
            "/v1/messages",
            Some(request.clone()),
            &[
                ("x-api-key", &key),
                ("anthropic-version", "2023-06-01"),
                ("anthropic-beta", "prompt-caching"),
            ],
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["type"], "message");
    assert_eq!(body["content"][0]["text"], "Hello world");
    let sent = upstream.last();
    assert_eq!(sent.body["model"], "cached-claude");
    assert_eq!(sent.api_key.as_deref(), Some("provider-secret"));
    let log = last_log(&app).await;
    assert_eq!(log.endpoint, "messages");
    assert_eq!((log.prompt_tokens, log.cached_tokens), (160, 100));

    let mut streaming = request;
    streaming["stream"] = json!(true);
    let (_, text) = app
        .call_text("/v1/messages", streaming, &[("x-api-key", &key)])
        .await;
    assert!(text.contains("event: message_start") && text.contains("text_delta"));
    let log = last_log(&app).await;
    assert_eq!((log.prompt_tokens, log.completion_tokens), (10, 20));
}

#[actix_web::test]
async fn messages_to_an_openai_model_are_translated_both_ways() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "gpt",
        "gpt-4o",
        json!({"input": 2.5, "output": 10.0}),
    )
    .await;

    let request = json!({"model": "gpt", "max_tokens": 100, "system": "Be terse.",
                         "messages": [{"role": "user", "content": "hi"}]});
    let (status, body) = app
        .call_with(
            "POST",
            "/v1/messages",
            Some(request.clone()),
            &[("x-api-key", &key)],
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["type"], "message");
    assert_eq!(body["content"][0]["text"], "Hello world");
    assert_eq!(body["stop_reason"], "end_turn");
    assert_eq!(body["usage"]["input_tokens"], 10);
    let sent = upstream.last();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.body["messages"][0],
        json!({"role": "system", "content": "Be terse."})
    );

    let mut streaming = request;
    streaming["stream"] = json!(true);
    let (_, text) = app
        .call_text("/v1/messages", streaming, &[("x-api-key", &key)])
        .await;
    let kinds: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("event: "))
        .collect();
    assert_eq!(kinds.first(), Some(&"message_start"));
    assert_eq!(kinds.last(), Some(&"message_stop"));
    assert!(text.contains("\"text\":\"Hello\""));
    assert_eq!(last_log(&app).await.completion_tokens, 20);
}

#[actix_web::test]
async fn count_tokens_asks_anthropic_or_estimates() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let claude = setup(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
        json!({"input": 3.0, "output": 15.0}),
    )
    .await;
    let gpt = setup(
        &mut app,
        &upstream,
        "openai",
        "gpt",
        "gpt-4o",
        json!({"input": 2.5, "output": 10.0}),
    )
    .await;
    let body = |model: &str| json!({"model": model, "messages": [{"role": "user", "content": "abcdefgh"}]});

    let (_, counted) = app
        .call_with(
            "POST",
            "/v1/messages/count_tokens",
            Some(body("claude")),
            &[("x-api-key", &claude)],
        )
        .await;
    assert_eq!(counted["input_tokens"], 42);
    let (_, estimated) = app
        .call_with(
            "POST",
            "/v1/messages/count_tokens",
            Some(body("gpt")),
            &[("x-api-key", &gpt)],
        )
        .await;
    assert_eq!(estimated["input_tokens"], 2);
}

#[actix_web::test]
async fn anthropic_gets_json_mode_and_thinking_translated() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
        json!({"input": 3.0, "output": 15.0}),
    )
    .await;

    let mut body = chat("claude");
    body["response_format"] = json!({"type": "json_object"});
    post(&mut app, "/v1/chat/completions", &key, body).await;
    let sent = upstream.last().body;
    assert_eq!(sent["tools"][0]["name"], "json_tool_call");
    assert_eq!(sent["tool_choice"]["name"], "json_tool_call");
    assert!(
        sent["max_tokens"].as_u64().unwrap() >= 64_000,
        "the catalog's limit for the model"
    );

    let mut body = chat("claude");
    body["reasoning_effort"] = json!("medium");
    post(&mut app, "/v1/chat/completions", &key, body).await;
    assert_eq!(upstream.last().body["thinking"]["budget_tokens"], 2048);
}

#[actix_web::test]
async fn providers_without_stream_usage_are_not_sent_stream_options() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "mistral",
        "mistral",
        "mistral-large-latest",
        json!({"input": 2.0, "output": 6.0}),
    )
    .await;

    let mut body = chat("mistral");
    body["stream"] = json!(true);
    stream(&mut app, "/v1/chat/completions", &key, body).await;
    assert!(upstream.last().body.get("stream_options").is_none());
    // Estimated from the text instead: "Hello world" is 11 characters.
    assert_eq!(last_log(&app).await.completion_tokens, 3);
}

#[actix_web::test]
async fn responses_reach_providers_without_them_through_chat_completions() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "groq",
        "llama",
        "llama-3.3-70b",
        json!({"input": 1.0, "output": 2.0}),
    )
    .await;

    let (status, body) = post(
        &mut app,
        "/v1/responses",
        &key,
        json!({"model": "llama", "instructions": "Be terse.", "input": "hi"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["object"], "response");
    assert_eq!(body["output"][0]["content"][0]["text"], "Hello world");
    assert_eq!(body["usage"]["input_tokens"], 10);
    let sent = upstream.last();
    assert_eq!(
        sent.path, "/v1/chat/completions",
        "translated, not passed through"
    );
    assert_eq!(sent.body["messages"][0]["content"], "Be terse.");
    assert_eq!(last_log(&app).await.endpoint, "responses");

    let text = stream(
        &mut app,
        "/v1/responses",
        &key,
        json!({"model": "llama", "input": "hi", "stream": true}),
    )
    .await;
    assert!(text.contains("event: response.output_text.delta"));
    assert!(text.contains("event: response.completed"));
    assert_eq!(last_log(&app).await.completion_tokens, 20);
}

#[actix_web::test]
async fn responses_reach_anthropic_through_two_translations() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "anthropic",
        "claude",
        "claude-sonnet-4-5",
        json!({"input": 3.0, "output": 15.0}),
    )
    .await;

    let (status, body) = post(&mut app, "/v1/responses", &key,
        json!({"model": "claude", "input": [{"role": "user", "content": [{"type": "input_text", "text": "hi"}]}]})).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["output"][0]["content"][0]["text"], "Hello world");
    assert_eq!(upstream.last().path, "/v1/messages");

    let text = stream(
        &mut app,
        "/v1/responses",
        &key,
        json!({"model": "claude", "input": "hi", "stream": true}),
    )
    .await;
    assert!(text.contains("\"delta\":\"Hello\""));
    assert!(text.contains("event: response.completed"));
    let log = last_log(&app).await;
    assert_eq!((log.prompt_tokens, log.completion_tokens), (10, 20));
}

#[actix_web::test]
async fn openai_keeps_its_native_responses_api() {
    let upstream = Upstream::start().await;
    let mut app = test_app().await;
    let key = setup(
        &mut app,
        &upstream,
        "openai",
        "o",
        "o4-mini",
        json!({"input": 1.0, "output": 4.0}),
    )
    .await;
    let (status, _) = post(
        &mut app,
        "/v1/responses",
        &key,
        json!({"model": "o", "input": "hi", "previous_response_id": "resp_0"}),
    )
    .await;
    assert_eq!(status, 200, "stateful requests pass through to OpenAI");
    assert_eq!(upstream.last().path, "/v1/responses");
}
