//! The surface applications call with a virtual key: OpenAI's API under
//! `/v1` (like OpenAI's own SDK base URLs), plus Anthropic's `/v1/messages`.

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use serde_json::{json, Map, Value};
use std::sync::Arc;

use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::gateway::forward::{self, AnthropicHeaders, Endpoint, Part};
use crate::gateway::{router, Caller, KeyContext, Slot};

/// Requests carry whole conversations, inline images and audio files.
const MAX_BODY: usize = 32 * 1024 * 1024;

/// The JSON endpoints, each one route.
const JSON_ENDPOINTS: [Endpoint; 7] = [
    Endpoint::ChatCompletions,
    Endpoint::Completions,
    Endpoint::Embeddings,
    Endpoint::Responses,
    Endpoint::ImageGenerations,
    Endpoint::AudioSpeech,
    Endpoint::Moderations,
];

pub fn configure(cfg: &mut web::ServiceConfig) {
    let mut v1 = web::scope("/v1")
        .default_service(web::to(super::not_found))
        .app_data(web::PayloadConfig::new(MAX_BODY))
        .route("/models", web::get().to(models))
        .route("/key/info", web::get().to(key_info))
        .route("/messages", web::post().to(messages))
        .route("/messages/count_tokens", web::post().to(count_tokens))
        .route(
            "/audio/transcriptions",
            web::post().to(|s, r, p| multipart(s, r, p, Endpoint::AudioTranscriptions)),
        )
        .route(
            "/audio/translations",
            web::post().to(|s, r, p| multipart(s, r, p, Endpoint::AudioTranslations)),
        );
    for endpoint in JSON_ENDPOINTS {
        v1 = v1.route(
            &format!("/{}", endpoint.path()),
            web::post().to(move |s, r, b| json_endpoint(s, r, b, endpoint)),
        );
    }
    cfg.service(v1)
        // The unversioned aliases are POST only: `GET /models` is a dashboard page.
        .service(
            web::resource("/chat/completions")
                .app_data(web::PayloadConfig::new(MAX_BODY))
                .route(web::post().to(|s, r, b| json_endpoint(s, r, b, Endpoint::ChatCompletions))),
        )
        .service(
            web::resource("/embeddings")
                .app_data(web::PayloadConfig::new(MAX_BODY))
                .route(web::post().to(|s, r, b| json_endpoint(s, r, b, Endpoint::Embeddings))),
        );
}

/// `Authorization: Bearer sk-...`, or `x-api-key: sk-...` as Anthropic's
/// SDKs send it. Nothing else: every extra place a key may hide is one more
/// place it leaks from.
fn raw_key(req: &HttpRequest) -> AppResult<String> {
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    header("authorization")
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
        .or_else(|| header("x-api-key"))
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            AppError::Unauthorized("missing API key; send `Authorization: Bearer sk-...`".into())
        })
}

async fn key_for(state: &AppState, req: &HttpRequest) -> AppResult<(Arc<KeyContext>, Slot)> {
    state.gateway.admit(&raw_key(req)?).await
}

/// `/key/info`: what a key may spend and how fast, read with the
/// key itself. Not a request against its limits.
async fn key_info(state: web::Data<AppState>, req: HttpRequest) -> AppResult<HttpResponse> {
    let key = state.gateway.authenticate(&raw_key(&req)?).await?;
    let mut info = crate::services::keys::get(&state.pool, key.key_id).await?;
    info.spend_usd = crate::models::money::nanos_to_usd(key.spend_nanos());
    Ok(HttpResponse::Ok().json(info))
}

const MAX_TAGS: usize = 20;
const MAX_TAG_LEN: usize = 64;
const MAX_END_USER_LEN: usize = 256;

fn bounded(value: &str, max: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.chars().take(max).collect())
}

/// Who the request is for, read from these fields. The end user:
/// `x-litellm-customer-id` or `x-litellm-end-user-id`, then `user`,
/// `litellm_metadata.user`, `metadata.user_id`, `safety_identifier`. Tags:
/// `x-litellm-tags` (comma-separated), then `tags`, `metadata.tags` and
/// `litellm_metadata.tags`. Tags and `litellm_metadata` are the gateway's,
/// so they are taken out of the body; `user` and `metadata.user_id` are the
/// provider's own fields and stay.
fn caller(
    (key, slot): (Arc<KeyContext>, Slot),
    req: &HttpRequest,
    body: &mut Map<String, Value>,
) -> Arc<Caller> {
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    let gateway_meta = body.remove("litellm_metadata").unwrap_or(Value::Null);
    let end_user = header("x-litellm-customer-id")
        .or_else(|| header("x-litellm-end-user-id"))
        .or_else(|| body.get("user").and_then(Value::as_str))
        .or_else(|| gateway_meta["user"].as_str())
        .or_else(|| body.get("metadata").and_then(|m| m["user_id"].as_str()))
        .or_else(|| body.get("safety_identifier").and_then(Value::as_str))
        .and_then(|user| bounded(user, MAX_END_USER_LEN));

    let mut tags: Vec<String> = Vec::new();
    let mut add = |tag: &str| {
        if let Some(tag) = bounded(tag, MAX_TAG_LEN) {
            if tags.len() < MAX_TAGS && !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    };
    header("x-litellm-tags")
        .into_iter()
        .flat_map(|h| h.split(','))
        .for_each(&mut add);
    let mut listed = |value: Option<Value>| {
        value
            .as_ref()
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .for_each(&mut add);
    };
    listed(body.remove("tags"));
    if let Some(Value::Object(metadata)) = body.get_mut("metadata") {
        listed(metadata.remove("tags"));
        if metadata.is_empty() {
            body.remove("metadata");
        }
    }
    listed(gateway_meta.get("tags").cloned());
    Arc::new(Caller {
        key,
        end_user,
        tags,
        slot,
    })
}

/// Authenticates the key, parses the body and checks its model.
async fn admit(
    state: &AppState,
    req: &HttpRequest,
    body: &[u8],
) -> AppResult<(Arc<Caller>, String, Map<String, Value>)> {
    let admitted = key_for(state, req).await?;
    let mut body: Map<String, Value> = serde_json::from_slice(body)
        .map_err(|e| AppError::Validation(format!("body is not a JSON object: {e}")))?;
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("`model` is required".into()))?
        .to_string();
    state.gateway.check_model(&admitted.0, &model).await?;
    Ok((caller(admitted, req, &mut body), model, body))
}

async fn json_endpoint(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Bytes,
    endpoint: Endpoint,
) -> AppResult<HttpResponse> {
    let (caller, model, body) = admit(&state, &req, &body).await?;
    let gateway = &state.gateway;
    router::run(gateway, &caller.key, &model, |route| {
        forward::openai_json(gateway, caller.clone(), route, endpoint, body.clone())
    })
    .await
}

fn anthropic_headers(req: &HttpRequest) -> AnthropicHeaders {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    };
    AnthropicHeaders {
        version: header("anthropic-version"),
        beta: header("anthropic-beta"),
    }
}

async fn messages(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Bytes,
) -> AppResult<HttpResponse> {
    let (caller, model, body) = admit(&state, &req, &body).await?;
    let gateway = &state.gateway;
    let headers = anthropic_headers(&req);
    router::run(gateway, &caller.key, &model, |route| {
        forward::messages(
            gateway,
            caller.clone(),
            route,
            body.clone(),
            headers.clone(),
        )
    })
    .await
}

async fn count_tokens(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Bytes,
) -> AppResult<HttpResponse> {
    let (caller, model, body) = admit(&state, &req, &body).await?;
    let route = state.gateway.any_route(&caller.key, &model).await?;
    forward::count_tokens(&state.gateway, route, body, anthropic_headers(&req)).await
}

/// Reads every part of the upload; the `model` part picks the route.
async fn multipart(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: Multipart,
    endpoint: Endpoint,
) -> AppResult<HttpResponse> {
    let admitted = key_for(&state, &req).await?;
    let mut parts = Vec::new();
    let mut size = 0;
    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|e| AppError::Validation(format!("bad upload: {e}")))?;
        let disposition = field.content_disposition().cloned();
        let name = disposition
            .as_ref()
            .and_then(|d| d.get_name())
            .unwrap_or_default()
            .to_string();
        let filename = disposition
            .as_ref()
            .and_then(|d| d.get_filename())
            .map(String::from);
        let content_type = field.content_type().map(|m| m.to_string());
        let mut data = web::BytesMut::new();
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|e| AppError::Validation(format!("bad upload: {e}")))?;
            size += chunk.len();
            if size > MAX_BODY {
                return Err(AppError::Validation("upload larger than 32 MB".into()));
            }
            data.extend_from_slice(&chunk);
        }
        parts.push(Part {
            name,
            filename,
            content_type,
            data: data.freeze(),
        });
    }
    let model = parts
        .iter()
        .find(|p| p.name == "model")
        .map(|p| String::from_utf8_lossy(&p.data).trim().to_string())
        .ok_or_else(|| AppError::Validation("`model` is required".into()))?;
    state.gateway.check_model(&admitted.0, &model).await?;
    let caller = caller(admitted, &req, &mut Map::new());
    let gateway = &state.gateway;
    router::run(gateway, &caller.key, &model, |route| {
        forward::openai_multipart(gateway, caller.clone(), route, endpoint, parts.clone())
    })
    .await
}

async fn models(state: web::Data<AppState>, req: HttpRequest) -> AppResult<HttpResponse> {
    let key = state.gateway.authenticate(&raw_key(&req)?).await?;
    let data: Vec<Value> = state
        .gateway
        .allowed_routes(&key)
        .await?
        .iter()
        .map(|route| {
            json!({
                "id": route.name,
                "object": "model",
                "created": route.created_at.timestamp(),
                "owned_by": route.provider,
            })
        })
        .collect();
    Ok(HttpResponse::Ok().json(json!({ "object": "list", "data": data })))
}
