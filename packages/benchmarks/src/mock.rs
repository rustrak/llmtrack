//! A provider that is never the bottleneck: OpenAI's chat completions with
//! usage, streaming or not, after an optional fixed delay. With the delay at
//! zero, whatever latency a gateway adds is all that is left to measure.

use actix_web::{web, App, HttpResponse, HttpServer};
use bytes::Bytes;
use futures_util::stream;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct MockConfig {
    /// Delay before answering (or before the first chunk).
    pub latency: Duration,
    /// Chunks in a streamed answer.
    pub chunks: usize,
    /// Delay between chunks.
    pub chunk_delay: Duration,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            latency: Duration::ZERO,
            chunks: 20,
            chunk_delay: Duration::ZERO,
        }
    }
}

fn usage(chunks: usize) -> Value {
    json!({"prompt_tokens": 50, "completion_tokens": chunks, "total_tokens": 50 + chunks})
}

async fn chat(config: web::Data<MockConfig>, body: web::Json<Value>) -> HttpResponse {
    let config = **config;
    let model = body["model"].as_str().unwrap_or("mock").to_string();
    tokio::time::sleep(config.latency).await;
    if body["stream"] != true {
        return HttpResponse::Ok().json(json!({
            "id": "chatcmpl-mock", "object": "chat.completion", "created": 0, "model": model,
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "token ".repeat(config.chunks)}, "finish_reason": "stop"}],
            "usage": usage(config.chunks),
        }));
    }
    let include_usage = body["stream_options"]["include_usage"] == true;
    let chunk = move |content: Value, finish: Value| {
        Bytes::from(format!(
            "data: {}\n\n",
            json!({"id": "chatcmpl-mock", "object": "chat.completion.chunk", "model": model,
                   "choices": [{"index": 0, "delta": {"content": content}, "finish_reason": finish}]})
        ))
    };
    let mut events: Vec<Bytes> = (0..config.chunks)
        .map(|_| chunk(json!("token "), Value::Null))
        .collect();
    events.push(chunk(Value::Null, json!("stop")));
    if include_usage {
        events.push(Bytes::from(format!(
            "data: {}\n\n",
            json!({"id": "chatcmpl-mock", "object": "chat.completion.chunk", "choices": [], "usage": usage(config.chunks)})
        )));
    }
    events.push(Bytes::from_static(b"data: [DONE]\n\n"));
    let delay = config.chunk_delay;
    let body = stream::unfold(events.into_iter(), move |mut rest| async move {
        let next = rest.next()?;
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        Some((Ok::<_, actix_web::Error>(next), rest))
    });
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(body)
}

/// Serves the mock on `addr` until the process ends. Returns the bound port.
pub fn serve(
    addr: (&str, u16),
    config: MockConfig,
) -> std::io::Result<(u16, actix_web::dev::Server)> {
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config))
            .app_data(web::JsonConfig::default().limit(32 * 1024 * 1024))
            .route("/v1/chat/completions", web::post().to(chat))
            .route("/chat/completions", web::post().to(chat))
            .route("/health", web::get().to(HttpResponse::Ok))
    })
    .bind(addr)?;
    let port = server.addrs()[0].port();
    Ok((port, server.run()))
}
