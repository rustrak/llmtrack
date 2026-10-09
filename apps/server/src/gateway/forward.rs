//! Calling the provider and metering what came back.
//!
//! Every endpoint takes the same path: swap the public model name for the
//! provider's, send it with the provider's credentials, pass the answer back
//! (translated when the dialects differ) and read the usage out of it on the
//! way. Streams are forwarded event by event and metered at the end.

use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use bytes::Bytes;
use chrono::Utc;
use futures_util::stream::BoxStream;
use futures_util::{Stream, StreamExt};
use serde_json::{json, Map, Value};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use tokio::sync::mpsc;

use super::anthropic::{self, ModelFacts};
use super::capture::{self, Transcript};
use super::messages;
use super::providers::Wire;
use super::responses;
use super::sse::SseParser;
use super::usage::{Bodies, Message, UsageEvent};
use super::{Caller, Gateway, Route, Usage};
use crate::error::{AppError, AppResult};

/// The OpenAI-style endpoints the gateway proxies, plus Anthropic's Messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    ChatCompletions,
    Completions,
    Embeddings,
    Responses,
    ImageGenerations,
    AudioSpeech,
    AudioTranscriptions,
    AudioTranslations,
    Moderations,
    Messages,
}

impl Endpoint {
    /// The path under the provider's base URL, and the name logs use.
    pub fn path(self) -> &'static str {
        match self {
            Endpoint::ChatCompletions => "chat/completions",
            Endpoint::Completions => "completions",
            Endpoint::Embeddings => "embeddings",
            Endpoint::Responses => "responses",
            Endpoint::ImageGenerations => "images/generations",
            Endpoint::AudioSpeech => "audio/speech",
            Endpoint::AudioTranscriptions => "audio/transcriptions",
            Endpoint::AudioTranslations => "audio/translations",
            Endpoint::Moderations => "moderations",
            Endpoint::Messages => "messages",
        }
    }

    fn streams(self) -> bool {
        matches!(
            self,
            Endpoint::ChatCompletions
                | Endpoint::Completions
                | Endpoint::Responses
                | Endpoint::Messages
        )
    }
}

/// Records one request's usage, exactly once: explicitly when the response
/// is complete, or from `Drop` when the client went away mid-stream.
pub struct Meter {
    usage: mpsc::Sender<Message>,
    caller: Arc<Caller>,
    route: Arc<Route>,
    endpoint: Endpoint,
    started: Instant,
    request_id: String,
    stream: bool,
    /// Only for keys that keep them.
    bodies: Option<Box<Bodies>>,
}

impl Meter {
    pub fn new(
        gateway: &Gateway,
        caller: Arc<Caller>,
        route: Arc<Route>,
        endpoint: Endpoint,
        stream: bool,
    ) -> Self {
        Self {
            usage: gateway.usage_sender(),
            caller,
            route,
            endpoint,
            started: Instant::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
            stream,
            bodies: None,
        }
    }

    /// Keeps what the client sent, if its key keeps bodies.
    pub fn with_request(mut self, request: impl FnOnce() -> Value) -> Self {
        if self.caller.key.log_bodies {
            self.bodies = Some(Box::new(Bodies {
                request: request(),
                response: None,
            }));
        }
        self
    }

    /// Keeps the reply, if the request is being kept.
    fn reply(&mut self, reply: &Value) {
        if let Some(bodies) = self.bodies.as_mut().filter(|_| !reply.is_null()) {
            bodies.response = Some(reply.clone());
        }
    }

    /// Charges the counters now and builds the event for the writer.
    fn event(&mut self, status: u16, usage: Usage, error: Option<String>) -> UsageEvent {
        let cost = self.route.pricing.cost_nanos(&usage);
        self.caller
            .key
            .charge(cost, usage.prompt_tokens + usage.completion_tokens);
        UsageEvent {
            request_id: self.request_id.clone(),
            key_id: self.caller.key.key_id,
            team_id: self.caller.key.team_id,
            user_id: self.caller.key.user_id,
            model_id: self.route.id,
            end_user: self.caller.end_user.clone(),
            tags: self.caller.tags.clone(),
            model_name: self.route.name.clone(),
            provider: self.route.provider.clone(),
            endpoint: self.endpoint.path(),
            status_code: status,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cached_tokens: usage.cached_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            cost_nanos: cost,
            latency_ms: i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX),
            stream: self.stream,
            error,
            at: Utc::now(),
            bodies: self.bodies.take(),
        }
    }

    pub async fn record(mut self, status: u16, usage: Usage, error: Option<String>) {
        let event = self.event(status, usage, error);
        if self
            .usage
            .send(Message::Event(Box::new(event)))
            .await
            .is_err()
        {
            log::error!(
                "usage writer is gone; request {} not logged",
                self.request_id
            );
        }
    }

    /// For `Drop`, which cannot await.
    fn record_detached(mut self, status: u16, usage: Usage, error: Option<String>) {
        let event = self.event(status, usage, error);
        let sender = self.usage.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move {
                    let _ = sender.send(Message::Event(Box::new(event))).await;
                });
            }
            Err(_) => log::error!("no runtime to log request {}", event.request_id),
        }
    }
}

fn no_such_endpoint(route: &Route, endpoint: Endpoint) -> AppError {
    AppError::Validation(format!(
        "model '{}' is served by Anthropic, which has no /v1/{}",
        route.name,
        endpoint.path()
    ))
}

fn content_type_of(response: &reqwest::Response) -> String {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string()
}

/// A JSON endpoint in OpenAI's dialect.
pub async fn openai_json(
    gateway: &Gateway,
    caller: Arc<Caller>,
    route: Arc<Route>,
    endpoint: Endpoint,
    mut body: Map<String, Value>,
) -> AppResult<HttpResponse> {
    let stream = endpoint.streams() && body.get("stream") == Some(&Value::Bool(true));
    let request = || Value::Object(body.clone());
    if endpoint == Endpoint::Responses && !route.native_responses {
        let meter =
            Meter::new(gateway, caller, route.clone(), endpoint, stream).with_request(request);
        return responses_via_chat(gateway, meter, &route, body, stream).await;
    }
    if route.wire == Wire::Anthropic {
        if endpoint != Endpoint::ChatCompletions {
            return Err(no_such_endpoint(&route, endpoint));
        }
        let meter =
            Meter::new(gateway, caller, route.clone(), endpoint, stream).with_request(request);
        return anthropic_chat(gateway, meter, &route, body, stream).await;
    }
    let mut meter =
        Meter::new(gateway, caller, route.clone(), endpoint, stream).with_request(request);

    let prompt_estimate = estimate_tokens(&Value::Object(body.clone()));
    let characters = match endpoint {
        Endpoint::AudioSpeech => body
            .get("input")
            .and_then(Value::as_str)
            .map_or(0, |text| text.chars().count() as i64),
        _ => 0,
    };
    body.insert("model".into(), Value::String(route.upstream_model.clone()));
    let client_wants_usage = body
        .get("stream_options")
        .is_some_and(|o| o["include_usage"] == true);
    let injects_usage = stream
        && matches!(endpoint, Endpoint::ChatCompletions | Endpoint::Completions)
        && route.stream_usage
        && !client_wants_usage;
    if injects_usage {
        // The only reliable token count a stream carries; stripped again on
        // the way back.
        body.entry("stream_options")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .map(|o| o.insert("include_usage".into(), Value::Bool(true)));
    }

    let request = openai_request(gateway, &route, endpoint.path()).json(&body);
    let response = match send(request).await {
        Ok(response) => response,
        Err(e) => return fail(meter, StatusCode::BAD_GATEWAY, e).await,
    };
    if stream && response.status().is_success() {
        let mode = Mode::OpenAi {
            endpoint,
            strip_usage: injects_usage,
            usage_seen: false,
            completion_chars: 0,
            prompt_estimate,
        };
        return Ok(sse_response(Metered::new(response, mode, meter)));
    }
    let content_type = content_type_of(&response);
    let (status, bytes) = read(response).await?;
    if !status.is_success() {
        return upstream_error(meter, status, &bytes, |b| b.clone()).await;
    }
    let reply = parse(&bytes).unwrap_or(Value::Null);
    let usage = match endpoint {
        Endpoint::Responses => Usage::from_responses(&reply["usage"]).unwrap_or_default(),
        Endpoint::ImageGenerations => {
            Usage::from_responses(&reply["usage"]).unwrap_or_else(|| Usage {
                images: reply["data"].as_array().map_or(0, |d| d.len() as i64),
                ..Usage::default()
            })
        }
        Endpoint::AudioSpeech => Usage {
            characters,
            ..Usage::default()
        },
        _ => Usage::from_openai(&reply["usage"]).unwrap_or_default(),
    };
    // ponytail: embeddings keep their input only; the vectors are of no use to read.
    if endpoint != Endpoint::Embeddings {
        meter.reply(&reply);
    }
    meter.record(status.as_u16(), usage, None).await;
    Ok(HttpResponse::build(status)
        .content_type(content_type)
        .body(bytes))
}

/// The Responses API for a provider without one: as a chat completion, to
/// Anthropic (translated once more) or to an OpenAI-dialect provider.
async fn responses_via_chat(
    gateway: &Gateway,
    mut meter: Meter,
    route: &Route,
    body: Map<String, Value>,
    stream: bool,
) -> AppResult<HttpResponse> {
    let mut chat = responses::to_chat_request(&body, &route.upstream_model)?;
    let (request, json_mode) = if route.wire == Wire::Anthropic {
        let chat = chat.as_object().cloned().unwrap_or_default();
        let facts = ModelFacts {
            max_output_tokens: route.max_output_tokens,
        };
        let (translated, json_mode) =
            anthropic::to_anthropic_request(&chat, &route.upstream_model, facts)?;
        let request = anthropic_request(gateway, route, "messages", &AnthropicHeaders::default())
            .json(&translated);
        (request, json_mode)
    } else {
        if stream && route.stream_usage {
            chat["stream_options"] = json!({"include_usage": true});
        }
        (
            openai_request(gateway, route, "chat/completions").json(&chat),
            false,
        )
    };
    let response = match send(request).await {
        Ok(response) => response,
        Err(e) => return fail(meter, StatusCode::BAD_GATEWAY, e).await,
    };
    if stream && response.status().is_success() {
        let translator = Box::new(responses::StreamTranslator::new(&route.name));
        let mode = if route.wire == Wire::Anthropic {
            let from = anthropic::StreamTranslator::new(&route.name, true, json_mode);
            Mode::AnthropicToResponses(Box::new(from), translator)
        } else {
            Mode::ToResponses(translator)
        };
        return Ok(sse_response(Metered::new(response, mode, meter)));
    }
    let (status, bytes) = read(response).await?;
    if !status.is_success() {
        let reshape = if route.wire == Wire::Anthropic {
            anthropic::to_openai_error
        } else {
            |b: &Value| b.clone()
        };
        return upstream_error(meter, status, &bytes, reshape).await;
    }
    let reply = parse(&bytes).ok_or_else(|| {
        AppError::Upstream("the provider answered with something other than JSON".into())
    })?;
    let completion = if route.wire == Wire::Anthropic {
        anthropic::to_openai_response(&reply, &route.name, json_mode).0
    } else {
        reply
    };
    let (response, usage) = responses::to_response(&completion, &route.name);
    meter.reply(&response);
    meter.record(status.as_u16(), usage, None).await;
    Ok(HttpResponse::Ok().json(response))
}

/// What a connection test found.
#[derive(Debug, serde::Serialize)]
pub struct Probe {
    pub ok: bool,
    /// The provider's status, or `None` when it could not be reached.
    pub status: Option<u16>,
    pub message: String,
    pub latency_ms: u64,
}

/// "Test Connect": the smallest request the model answers, one token of chat
/// (or one embedding), sent with the settings as they are, saved or not.
pub async fn probe(gateway: &Gateway, route: &Route, embedding: bool) -> Probe {
    let ping = json!([{"role": "user", "content": "ping"}]);
    let request = match (route.wire, embedding) {
        (Wire::Anthropic, _) => {
            anthropic_request(gateway, route, "messages", &AnthropicHeaders::default())
                .json(&json!({"model": route.upstream_model, "max_tokens": 1, "messages": ping}))
        }
        (_, true) => openai_request(gateway, route, "embeddings")
            .json(&json!({"model": route.upstream_model, "input": "ping"})),
        (_, false) => openai_request(gateway, route, "chat/completions")
            .json(&json!({"model": route.upstream_model, "max_tokens": 1, "messages": ping})),
    };
    let started = Instant::now();
    let result = request.send().await;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match result {
        Err(e) => Probe {
            ok: false,
            status: None,
            message: e.to_string(),
            latency_ms,
        },
        Ok(response) => {
            let status = response.status().as_u16();
            let bytes = response.bytes().await.unwrap_or_default();
            let message = parse(&bytes)
                .and_then(|b| b["error"]["message"].as_str().map(String::from))
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(300).collect());
            let ok = (200..300).contains(&status);
            Probe {
                ok,
                status: Some(status),
                message: if ok { "ok".into() } else { message },
                latency_ms,
            }
        }
    }
}

/// One part of a multipart upload, kept whole to be sent on.
#[derive(Clone)]
pub struct Part {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Bytes,
}

/// Audio transcription and translation: multipart in, multipart out, with
/// the `model` part rewritten.
pub async fn openai_multipart(
    gateway: &Gateway,
    caller: Arc<Caller>,
    route: Arc<Route>,
    endpoint: Endpoint,
    parts: Vec<Part>,
) -> AppResult<HttpResponse> {
    if route.wire == Wire::Anthropic {
        return Err(no_such_endpoint(&route, endpoint));
    }
    let mut meter = Meter::new(gateway, caller, route.clone(), endpoint, false)
        .with_request(|| capture::multipart_request(&parts));
    let mut form = reqwest::multipart::Form::new();
    for part in parts {
        if part.name == "model" {
            form = form.text("model", route.upstream_model.clone());
            continue;
        }
        let mut field = reqwest::multipart::Part::bytes(part.data.to_vec());
        if let Some(filename) = part.filename {
            field = field.file_name(filename);
        }
        if let Some(content_type) = part.content_type {
            field = field
                .mime_str(&content_type)
                .map_err(|_| AppError::Validation(format!("bad content type '{content_type}'")))?;
        }
        form = form.part(part.name, field);
    }
    let request = openai_request(gateway, &route, endpoint.path()).multipart(form);
    let response = match send(request).await {
        Ok(response) => response,
        Err(e) => return fail(meter, StatusCode::BAD_GATEWAY, e).await,
    };
    let content_type = content_type_of(&response);
    let (status, bytes) = read(response).await?;
    if !status.is_success() {
        return upstream_error(meter, status, &bytes, |b| b.clone()).await;
    }
    // Token-billed models report tokens, duration-billed ones seconds; the
    // verbose format carries a `duration`.
    let reply = parse(&bytes).unwrap_or(Value::Null);
    let usage = &reply["usage"];
    let seconds = usage["seconds"]
        .as_f64()
        .or_else(|| reply["duration"].as_f64())
        .map_or(0, |s| s.ceil() as i64);
    let usage = match Usage::from_responses(usage) {
        Some(tokens) => Usage { seconds, ..tokens },
        None => Usage {
            seconds,
            ..Usage::default()
        },
    };
    meter.reply(&reply);
    meter.record(status.as_u16(), usage, None).await;
    Ok(HttpResponse::build(status)
        .content_type(content_type)
        .body(bytes))
}

/// The headers an Anthropic client sends that Anthropic needs to see.
#[derive(Debug, Default, Clone)]
pub struct AnthropicHeaders {
    pub version: Option<String>,
    pub beta: Option<String>,
}

/// `POST /v1/messages`: Anthropic's own API. Native for Anthropic models,
/// translated through chat completions for everyone else.
pub async fn messages(
    gateway: &Gateway,
    caller: Arc<Caller>,
    route: Arc<Route>,
    mut body: Map<String, Value>,
    headers: AnthropicHeaders,
) -> AppResult<HttpResponse> {
    let stream = body.get("stream") == Some(&Value::Bool(true));
    let mut meter = Meter::new(gateway, caller, route.clone(), Endpoint::Messages, stream)
        .with_request(|| Value::Object(body.clone()));

    if route.wire == Wire::Anthropic {
        body.insert("model".into(), Value::String(route.upstream_model.clone()));
        let request = anthropic_request(gateway, &route, "messages", &headers).json(&body);
        let response = match send(request).await {
            Ok(response) => response,
            Err(e) => return fail(meter, StatusCode::BAD_GATEWAY, e).await,
        };
        if stream && response.status().is_success() {
            return Ok(sse_response(Metered::new(
                response,
                Mode::AnthropicRaw,
                meter,
            )));
        }
        let (status, bytes) = read(response).await?;
        if !status.is_success() {
            return upstream_error(meter, status, &bytes, |b| b.clone()).await;
        }
        let reply = parse(&bytes).unwrap_or(Value::Null);
        meter.reply(&reply);
        meter
            .record(
                status.as_u16(),
                Usage::from_anthropic(&reply["usage"]),
                None,
            )
            .await;
        return Ok(HttpResponse::build(status)
            .content_type("application/json")
            .body(bytes));
    }

    let mut chat = messages::to_openai_request(&body, &route.upstream_model);
    if stream && route.stream_usage {
        chat["stream_options"] = json!({"include_usage": true});
    }
    let request = openai_request(gateway, &route, "chat/completions").json(&chat);
    let response = match send(request).await {
        Ok(response) => response,
        Err(e) => return fail(meter, StatusCode::BAD_GATEWAY, e).await,
    };
    if stream && response.status().is_success() {
        let mode = Mode::ToAnthropic(Box::new(messages::StreamTranslator::new(&route.name)));
        return Ok(sse_response(Metered::new(response, mode, meter)));
    }
    let (status, bytes) = read(response).await?;
    if !status.is_success() {
        return upstream_error(meter, status, &bytes, messages::to_anthropic_error).await;
    }
    let completion = parse(&bytes).ok_or_else(|| {
        AppError::Upstream("the provider answered with something other than JSON".into())
    })?;
    let (reply, usage) = messages::to_anthropic_response(&completion, &route.name);
    meter.reply(&reply);
    meter.record(status.as_u16(), usage, None).await;
    Ok(HttpResponse::Ok().json(reply))
}

/// `POST /v1/messages/count_tokens`: Anthropic answers for its models; for
/// anyone else it is an estimate, which is all Claude Code uses it for.
pub async fn count_tokens(
    gateway: &Gateway,
    route: Arc<Route>,
    mut body: Map<String, Value>,
    headers: AnthropicHeaders,
) -> AppResult<HttpResponse> {
    if route.wire != Wire::Anthropic {
        let chat = messages::to_openai_request(&body, &route.upstream_model);
        return Ok(HttpResponse::Ok().json(json!({"input_tokens": estimate_tokens(&chat)})));
    }
    body.insert("model".into(), Value::String(route.upstream_model.clone()));
    let request = anthropic_request(gateway, &route, "messages/count_tokens", &headers).json(&body);
    let response = send(request).await.map_err(AppError::Upstream)?;
    let (status, bytes) = read(response).await?;
    Ok(HttpResponse::build(status)
        .content_type("application/json")
        .body(bytes))
}

/// A request in OpenAI's dialect to `path`, with the URL and credentials the
/// provider's flavour of it expects.
fn openai_request(gateway: &Gateway, route: &Route, path: &str) -> reqwest::RequestBuilder {
    match route.wire {
        Wire::Azure => {
            // The version is validated to URL-safe characters on save.
            let url = format!(
                "{}/openai/deployments/{}/{path}?api-version={}",
                route.api_base,
                route.upstream_model,
                route.api_version.as_deref().unwrap_or_default()
            );
            let request = gateway.http.post(url);
            match &route.api_key {
                Some(api_key) => request.header("api-key", api_key),
                None => request,
            }
        }
        _ => {
            let request = gateway.http.post(format!("{}/{path}", route.api_base));
            match &route.api_key {
                Some(api_key) => request.bearer_auth(api_key),
                None => request,
            }
        }
    }
}

fn anthropic_request(
    gateway: &Gateway,
    route: &Route,
    path: &str,
    headers: &AnthropicHeaders,
) -> reqwest::RequestBuilder {
    let mut request = gateway
        .http
        .post(format!("{}/{path}", route.api_base))
        .header(
            "anthropic-version",
            headers.version.as_deref().unwrap_or(anthropic::API_VERSION),
        );
    if let Some(beta) = &headers.beta {
        request = request.header("anthropic-beta", beta);
    }
    match &route.api_key {
        Some(api_key) => request.header("x-api-key", api_key),
        None => request,
    }
}

async fn anthropic_chat(
    gateway: &Gateway,
    mut meter: Meter,
    route: &Route,
    body: Map<String, Value>,
    stream: bool,
) -> AppResult<HttpResponse> {
    let include_usage = body
        .get("stream_options")
        .is_some_and(|o| o["include_usage"] == true);
    let facts = ModelFacts {
        max_output_tokens: route.max_output_tokens,
    };
    let (translated, json_mode) =
        anthropic::to_anthropic_request(&body, &route.upstream_model, facts)?;
    let request = anthropic_request(gateway, route, "messages", &AnthropicHeaders::default())
        .json(&translated);
    let response = match send(request).await {
        Ok(response) => response,
        Err(e) => return fail(meter, StatusCode::BAD_GATEWAY, e).await,
    };
    if stream && response.status().is_success() {
        let translator = anthropic::StreamTranslator::new(&route.name, include_usage, json_mode);
        let mode = Mode::FromAnthropic(Box::new(translator));
        return Ok(sse_response(Metered::new(response, mode, meter)));
    }
    let (status, bytes) = read(response).await?;
    if !status.is_success() {
        return upstream_error(meter, status, &bytes, anthropic::to_openai_error).await;
    }
    let message = parse(&bytes).ok_or_else(|| {
        AppError::Upstream("anthropic answered with something other than JSON".into())
    })?;
    let (completion, usage) = anthropic::to_openai_response(&message, &route.name, json_mode);
    meter.reply(&completion);
    meter.record(status.as_u16(), usage, None).await;
    Ok(HttpResponse::Ok().json(completion))
}

async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    request.send().await.map_err(|e| {
        let kind = if e.is_timeout() {
            "timed out"
        } else {
            "unreachable"
        };
        format!("provider {kind}: {e}")
    })
}

async fn read(response: reqwest::Response) -> AppResult<(StatusCode, Bytes)> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|_| AppError::Upstream("provider sent an invalid status".into()))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Upstream(format!("provider response cut short: {e}")))?;
    Ok((status, bytes))
}

/// The provider could not be reached: logged as a 502.
async fn fail(meter: Meter, status: StatusCode, message: String) -> AppResult<HttpResponse> {
    meter
        .record(status.as_u16(), Usage::default(), Some(message.clone()))
        .await;
    Err(AppError::Provider {
        status: None,
        message,
        body: None,
    })
}

/// A provider error goes back to the client as-is (in the dialect the client
/// speaks), except 401 and 403: those mean the gateway's provider key is
/// wrong, and passing them through would tell the client its own key is.
async fn upstream_error(
    mut meter: Meter,
    status: StatusCode,
    bytes: &Bytes,
    reshape: impl Fn(&Value) -> Value,
) -> AppResult<HttpResponse> {
    let parsed = parse(bytes).unwrap_or_else(
        || json!({"error": {"message": String::from_utf8_lossy(bytes), "type": "provider_error"}}),
    );
    let message = parsed["error"]["message"]
        .as_str()
        .unwrap_or("provider error")
        .to_string();
    meter.reply(&parsed);
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        let message =
            format!("the provider refused the gateway's credentials ({status}): {message}");
        meter
            .record(502, Usage::default(), Some(message.clone()))
            .await;
        return Err(AppError::Provider {
            status: Some(status.as_u16()),
            message,
            body: None,
        });
    }
    meter
        .record(status.as_u16(), Usage::default(), Some(message.clone()))
        .await;
    Err(AppError::Provider {
        status: Some(status.as_u16()),
        message,
        body: Some(reshape(&parsed)),
    })
}

fn parse(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

fn sse_response(stream: Metered) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .streaming(stream)
}

/// Roughly four characters per token: only used when a provider streams
/// without ever reporting usage, or to answer `count_tokens` for a model
/// whose provider has no such endpoint.
fn estimate_tokens(value: &Value) -> i64 {
    fn chars(value: &Value) -> u64 {
        match value {
            Value::String(s) => s.chars().count() as u64,
            Value::Array(items) => items.iter().map(chars).sum(),
            Value::Object(map) => ["messages", "content", "text", "input", "instructions"]
                .iter()
                .filter_map(|k| map.get(*k))
                .map(chars)
                .sum(),
            _ => 0,
        }
    }
    chars(value).div_ceil(4) as i64
}

enum Mode {
    /// The provider's own stream, forwarded event by event.
    OpenAi {
        endpoint: Endpoint,
        /// Drop the usage-only chunk the gateway asked for.
        strip_usage: bool,
        usage_seen: bool,
        completion_chars: i64,
        prompt_estimate: i64,
    },
    /// Anthropic's stream as chat completion chunks.
    FromAnthropic(Box<anthropic::StreamTranslator>),
    /// A chat completion stream as Anthropic events.
    ToAnthropic(Box<messages::StreamTranslator>),
    /// Anthropic's stream to an Anthropic client, untouched.
    AnthropicRaw,
    /// A chat completion stream as Responses API events.
    ToResponses(Box<responses::StreamTranslator>),
    /// Anthropic's stream as chat chunks, those as Responses API events.
    AnthropicToResponses(
        Box<anthropic::StreamTranslator>,
        Box<responses::StreamTranslator>,
    ),
}

/// The provider's stream, rewritten event by event and metered at the end.
struct Metered {
    inner: BoxStream<'static, Result<Bytes, reqwest::Error>>,
    parser: SseParser,
    mode: Mode,
    usage: Usage,
    meter: Option<Meter>,
    /// What the client was sent, for a key that keeps bodies.
    transcript: Option<Transcript>,
    done: bool,
}

impl Metered {
    fn new(response: reqwest::Response, mode: Mode, meter: Meter) -> Self {
        Self {
            transcript: meter
                .bodies
                .is_some()
                .then(|| Transcript::new(meter.endpoint)),
            inner: response.bytes_stream().boxed(),
            parser: SseParser::default(),
            mode,
            usage: Usage::default(),
            meter: Some(meter),
            done: false,
        }
    }

    /// The bytes to forward for what just arrived.
    fn process(&mut self, bytes: &[u8]) -> Vec<u8> {
        let events = self.parser.push(bytes);
        let mut out = Vec::new();
        for event in events {
            self.process_event(&event.raw, &event.data, &mut out);
        }
        out
    }

    fn process_event(&mut self, raw: &[u8], data: &str, out: &mut Vec<u8>) {
        match &mut self.mode {
            Mode::OpenAi {
                endpoint,
                strip_usage,
                usage_seen,
                completion_chars,
                ..
            } => {
                if let Ok(chunk) = serde_json::from_str::<Value>(data) {
                    if *endpoint == Endpoint::Responses {
                        if chunk["type"] == "response.completed" {
                            if let Some(usage) = Usage::from_responses(&chunk["response"]["usage"])
                            {
                                self.usage = usage;
                                *usage_seen = true;
                            }
                        }
                        if let Some(text) = chunk["delta"].as_str() {
                            *completion_chars += text.chars().count() as i64;
                        }
                    } else {
                        if let Some(usage) = Usage::from_openai(&chunk["usage"]) {
                            self.usage = usage;
                            *usage_seen = true;
                            let usage_only = chunk["choices"].as_array().is_some_and(Vec::is_empty);
                            if usage_only && *strip_usage {
                                return;
                            }
                        }
                        let choice = &chunk["choices"][0];
                        let text = choice["delta"]["content"]
                            .as_str()
                            .or_else(|| choice["text"].as_str());
                        if let Some(text) = text {
                            *completion_chars += text.chars().count() as i64;
                        }
                    }
                }
                out.extend_from_slice(raw);
            }
            Mode::FromAnthropic(translator) => {
                for payload in translator.on_event(data) {
                    out.extend_from_slice(b"data: ");
                    out.extend_from_slice(payload.as_bytes());
                    out.extend_from_slice(b"\n\n");
                }
                self.usage = translator.usage;
            }
            Mode::ToAnthropic(translator) => {
                out.extend_from_slice(translator.on_chunk(data).as_bytes());
                self.usage = translator.usage;
            }
            Mode::ToResponses(translator) => {
                out.extend_from_slice(translator.on_chunk(data).as_bytes());
                self.usage = translator.usage;
            }
            Mode::AnthropicToResponses(from, to) => {
                for payload in from.on_event(data) {
                    out.extend_from_slice(to.on_chunk(&payload).as_bytes());
                }
                self.usage = from.usage;
            }
            Mode::AnthropicRaw => {
                if let Ok(event) = serde_json::from_str::<Value>(data) {
                    match event["type"].as_str() {
                        Some("message_start") => {
                            self.usage = Usage::from_anthropic(&event["message"]["usage"]);
                        }
                        Some("message_delta") => {
                            let usage = &event["usage"];
                            if usage.get("input_tokens").is_some() {
                                self.usage = Usage::from_anthropic(usage);
                            } else if let Some(output) = usage["output_tokens"].as_i64() {
                                self.usage.completion_tokens = output;
                            }
                        }
                        _ => {}
                    }
                }
                out.extend_from_slice(raw);
            }
        }
    }

    fn final_usage(&self) -> Usage {
        match &self.mode {
            Mode::OpenAi {
                usage_seen: false,
                completion_chars,
                prompt_estimate,
                ..
            } => Usage {
                prompt_tokens: *prompt_estimate,
                completion_tokens: (completion_chars.max(&0) + 3) / 4,
                ..Usage::default()
            },
            _ => self.usage,
        }
    }

    /// The bytes on their way to the client.
    fn sent(&mut self, out: Vec<u8>) -> Poll<Option<Result<Bytes, actix_web::Error>>> {
        if let Some(transcript) = &mut self.transcript {
            transcript.push(&out);
        }
        Poll::Ready(Some(Ok(Bytes::from(out))))
    }

    fn finish(&mut self, status: u16, error: Option<String>) {
        let usage = self.final_usage();
        if let Some(mut meter) = self.meter.take() {
            if let Some(reply) = self.transcript.take().and_then(Transcript::finish) {
                meter.reply(&reply);
            }
            meter.record_detached(status, usage, error);
        }
    }
}

impl Stream for Metered {
    type Item = Result<Bytes, actix_web::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.done {
                return Poll::Ready(None);
            }
            match self.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(bytes))) => {
                    let out = self.process(&bytes);
                    if !out.is_empty() {
                        return self.sent(out);
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    self.done = true;
                    self.finish(502, Some(format!("provider stream broke: {e}")));
                    let error = json!({"error": {"message": "the provider stream was interrupted", "type": "UpstreamError"}});
                    return Poll::Ready(Some(Ok(Bytes::from(format!("data: {error}\n\n")))));
                }
                Poll::Ready(None) => {
                    self.done = true;
                    let mut out = Vec::new();
                    if let Some(event) = self.parser.finish() {
                        self.process_event(&event.raw, &event.data, &mut out);
                    }
                    match &mut self.mode {
                        Mode::ToAnthropic(translator) => {
                            out.extend_from_slice(translator.finish().as_bytes());
                        }
                        Mode::ToResponses(translator)
                        | Mode::AnthropicToResponses(_, translator) => {
                            out.extend_from_slice(translator.finish().as_bytes());
                        }
                        _ => {}
                    }
                    if let Some(transcript) = &mut self.transcript {
                        transcript.push(&out);
                    }
                    self.finish(200, None);
                    if !out.is_empty() {
                        return Poll::Ready(Some(Ok(Bytes::from(out))));
                    }
                }
            }
        }
    }
}

impl Drop for Metered {
    fn drop(&mut self) {
        // Still holding the meter means the client left before the end. The
        // provider bills what it generated, so we do too.
        if self.meter.is_some() {
            self.finish(499, Some("client closed the connection".into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_estimate_counts_message_text() {
        let body = json!({"messages": [{"role": "user", "content": "abcdefgh"},
                                      {"role": "user", "content": [{"type": "text", "text": "abcd"}]}]});
        assert_eq!(estimate_tokens(&body), 3);
    }
}
