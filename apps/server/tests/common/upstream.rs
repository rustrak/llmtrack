//! A fake provider on a real socket. It speaks just enough OpenAI and
//! Anthropic to be proxied to, and remembers every request it was sent.
//!
//! The upstream model name scripts the answer: `fail-500` answers 500,
//! `bad-key` answers 401, `rate-limited` 429, `too-long` a context-window
//! 400, `unsafe` a content-policy 400, `flaky-once` 500 on its first call
//! and success after; `slow` succeeds after 300 ms; anything else succeeds with 10 prompt tokens and 20
//! completion tokens.

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Captured {
    pub path: String,
    pub authorization: Option<String>,
    pub api_key: Option<String>,
    pub anthropic_version: Option<String>,
    /// Azure's `api-key` header.
    pub azure_key: Option<String>,
    pub query: String,
    pub body: Value,
}

#[derive(Clone)]
pub struct Upstream {
    pub base: String,
    pub requests: Arc<Mutex<Vec<Captured>>>,
}

impl Upstream {
    pub async fn start() -> Self {
        let requests: Arc<Mutex<Vec<Captured>>> = Arc::default();
        let shared = requests.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(shared.clone()))
                .route("/v1/chat/completions", web::post().to(openai_chat))
                .route(
                    "/openai/deployments/{deployment}/chat/completions",
                    web::post().to(openai_chat),
                )
                .route("/v1/embeddings", web::post().to(openai_embeddings))
                .route("/v1/completions", web::post().to(openai_completions))
                .route("/v1/responses", web::post().to(openai_responses))
                .route("/v1/images/generations", web::post().to(openai_images))
                .route("/v1/audio/speech", web::post().to(openai_speech))
                .route("/v1/moderations", web::post().to(openai_moderations))
                .route(
                    "/v1/audio/transcriptions",
                    web::post().to(openai_transcriptions),
                )
                .route("/v1/messages", web::post().to(anthropic_messages))
                .route("/catalog.json", web::get().to(price_list))
                .route(
                    "/v1/messages/count_tokens",
                    web::post().to(anthropic_count_tokens),
                )
        })
        .workers(1)
        .bind(("127.0.0.1", 0))
        .unwrap();
        let port = server.addrs()[0].port();
        actix_web::rt::spawn(server.run());
        Upstream {
            base: format!("http://127.0.0.1:{port}/v1"),
            requests,
        }
    }

    pub fn last(&self) -> Captured {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("no request reached the upstream")
    }

    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

fn capture(req: &HttpRequest, body: &Value) {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_string())
    };
    let requests = req
        .app_data::<web::Data<Arc<Mutex<Vec<Captured>>>>>()
        .unwrap();
    requests.lock().unwrap().push(Captured {
        path: req.path().to_string(),
        authorization: header("authorization"),
        api_key: header("x-api-key"),
        anthropic_version: header("anthropic-version"),
        azure_key: header("api-key"),
        query: req.query_string().to_string(),
        body: body.clone(),
    });
}

fn scripted_failure(model: &str) -> Option<HttpResponse> {
    match model {
        "rate-limited" => Some(
            HttpResponse::TooManyRequests()
                .json(json!({"error": {"message": "Rate limit reached", "type": "rate_limit_error"}})),
        ),
        "too-long" => Some(HttpResponse::BadRequest().json(json!({"error": {
            "message": "This model's maximum context length is 8192 tokens.",
            "type": "invalid_request_error", "code": "context_length_exceeded"}}))),
        "unsafe" => Some(HttpResponse::BadRequest().json(json!({"error": {
            "message": "Your request was rejected by the safety system.",
            "type": "invalid_request_error", "code": "content_policy_violation"}}))),
        "fail-500" => Some(
            HttpResponse::InternalServerError()
                .json(json!({"error": {"message": "the provider fell over", "type": "server_error"}})),
        ),
        "bad-key" => Some(
            HttpResponse::Unauthorized()
                .json(json!({"error": {"message": "Incorrect API key provided", "type": "invalid_request_error"}})),
        ),
        _ => None,
    }
}

/// How many requests for `model` this upstream has seen, this one included.
fn calls_to(req: &HttpRequest, model: &str) -> usize {
    req.app_data::<web::Data<Arc<Mutex<Vec<Captured>>>>>()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.body["model"] == model)
        .count()
}

fn sse(events: Vec<Value>, done: bool) -> HttpResponse {
    let mut body = String::new();
    for event in events {
        body.push_str(&format!("data: {event}\n\n"));
    }
    if done {
        body.push_str("data: [DONE]\n\n");
    }
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .body(body)
}

async fn openai_chat(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    let model = body["model"].as_str().unwrap_or_default().to_string();
    if let Some(failure) = scripted_failure(&model) {
        return failure;
    }
    if model == "slow" {
        actix_web::rt::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    if model == "flaky-once" && calls_to(&req, &model) == 1 {
        return scripted_failure("fail-500").unwrap();
    }
    let usage = if model == "cached-model" {
        json!({"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30,
               "prompt_tokens_details": {"cached_tokens": 6},
               "completion_tokens_details": {"reasoning_tokens": 5}})
    } else {
        json!({"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30})
    };
    if body["stream"] == true {
        let chunk = |delta: Value, finish: Value| {
            json!({"id": "chatcmpl-1", "object": "chat.completion.chunk", "model": model,
                   "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
        };
        let mut events = vec![
            chunk(json!({"role": "assistant", "content": ""}), Value::Null),
            chunk(json!({"content": "Hello"}), Value::Null),
            chunk(json!({"content": " world"}), Value::Null),
            chunk(json!({}), json!("stop")),
        ];
        if body["stream_options"]["include_usage"] == true {
            events.push(
                json!({"id": "chatcmpl-1", "object": "chat.completion.chunk",
                               "model": model, "choices": [], "usage": usage}),
            );
        }
        return sse(events, true);
    }
    HttpResponse::Ok().json(json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 1,
        "model": model,
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hello world"},
                     "finish_reason": "stop"}],
        "usage": usage
    }))
}

async fn openai_embeddings(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    HttpResponse::Ok().json(json!({
        "object": "list",
        "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2]}],
        "model": body["model"],
        "usage": {"prompt_tokens": 8, "total_tokens": 8}
    }))
}

async fn openai_completions(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    let usage = json!({"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30});
    if body["stream"] == true {
        let mut events = vec![
            json!({"id": "cmpl-1", "object": "text_completion", "choices": [{"index": 0, "text": "Hello"}]}),
            json!({"id": "cmpl-1", "object": "text_completion", "choices": [{"index": 0, "text": " world", "finish_reason": "stop"}]}),
        ];
        if body["stream_options"]["include_usage"] == true {
            events.push(
                json!({"id": "cmpl-1", "object": "text_completion", "choices": [], "usage": usage}),
            );
        }
        return sse(events, true);
    }
    HttpResponse::Ok().json(
        json!({"id": "cmpl-1", "object": "text_completion", "model": body["model"],
        "choices": [{"index": 0, "text": "Hello world", "finish_reason": "stop"}], "usage": usage}),
    )
}

async fn openai_responses(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    let usage = json!({"input_tokens": 10, "output_tokens": 20, "total_tokens": 30,
        "input_tokens_details": {"cached_tokens": 4}, "output_tokens_details": {"reasoning_tokens": 5}});
    let response = json!({"id": "resp_1", "object": "response", "status": "completed", "model": body["model"],
        "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Hello world"}]}],
        "usage": usage});
    if body["stream"] == true {
        let mut text = String::new();
        for (kind, data) in [
            (
                "response.created",
                json!({"type": "response.created", "response": {"id": "resp_1", "status": "in_progress"}}),
            ),
            (
                "response.output_text.delta",
                json!({"type": "response.output_text.delta", "delta": "Hello"}),
            ),
            (
                "response.output_text.delta",
                json!({"type": "response.output_text.delta", "delta": " world"}),
            ),
            (
                "response.completed",
                json!({"type": "response.completed", "response": response}),
            ),
        ] {
            text.push_str(&format!("event: {kind}\ndata: {data}\n\n"));
        }
        return HttpResponse::Ok()
            .content_type("text/event-stream")
            .body(text);
    }
    HttpResponse::Ok().json(response)
}

async fn openai_images(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    HttpResponse::Ok().json(
        json!({"created": 1, "data": [{"url": "https://img/1.png"}, {"url": "https://img/2.png"}]}),
    )
}

async fn openai_speech(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    HttpResponse::Ok()
        .content_type("audio/mpeg")
        .body(&b"ID3-fake-audio"[..])
}

async fn openai_moderations(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    HttpResponse::Ok().json(json!({"id": "modr-1", "results": [{"flagged": false}]}))
}

/// Multipart: the raw body is kept as a string so tests can look inside.
async fn openai_transcriptions(req: HttpRequest, body: web::Bytes) -> HttpResponse {
    capture(
        &req,
        &Value::String(String::from_utf8_lossy(&body).to_string()),
    );
    HttpResponse::Ok().json(json!({"text": "hello", "usage": {"type": "duration", "seconds": 3}}))
}

/// A price list where gpt-4o costs $5 / $20 per 1M.
async fn price_list() -> HttpResponse {
    let mut list = serde_json::Map::new();
    for i in 0..600 {
        list.insert(format!("filler-{i}"), json!({"input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6, "litellm_provider": "openai", "mode": "chat"}));
    }
    list.insert("gpt-4o".into(), json!({"input_cost_per_token": 5e-6, "output_cost_per_token": 2e-5, "litellm_provider": "openai", "mode": "chat"}));
    HttpResponse::Ok().json(Value::Object(list))
}

async fn anthropic_count_tokens(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    HttpResponse::Ok().json(json!({"input_tokens": 42}))
}

async fn anthropic_messages(req: HttpRequest, body: web::Json<Value>) -> HttpResponse {
    capture(&req, &body);
    let model = body["model"].as_str().unwrap_or_default().to_string();
    if model == "fail-500" {
        return HttpResponse::InternalServerError().json(
            json!({"type": "error", "error": {"type": "api_error", "message": "overloaded"}}),
        );
    }
    if body["stream"] == true {
        let events = vec![
            json!({"type": "message_start", "message": {"id": "msg_1", "type": "message", "role": "assistant",
                   "model": model, "content": [], "usage": {"input_tokens": 10, "output_tokens": 1}}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "ping"}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": " world"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 20}}),
            json!({"type": "message_stop"}),
        ];
        let mut body = String::new();
        for event in events {
            body.push_str(&format!(
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().unwrap()
            ));
        }
        return HttpResponse::Ok()
            .content_type("text/event-stream")
            .body(body);
    }
    HttpResponse::Ok().json(json!({
        "id": "msg_1",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": "Hello world"}],
        "stop_reason": "end_turn",
        "usage": if model == "cached-claude" {
            json!({"input_tokens": 10, "output_tokens": 20, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 50})
        } else {
            json!({"input_tokens": 10, "output_tokens": 20})
        }
    }))
}
