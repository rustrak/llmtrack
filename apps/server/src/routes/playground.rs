//! The dashboard's Playground: chat with a model through one of your keys,
//! signed in. The browser names the key by id and never holds it; the
//! request then takes the same path as any `/v1/chat/completions`, so the
//! key's limits apply and its spend, logs and person count it (tagged
//! `playground`).

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::sync::Arc;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::gateway::forward::{self, Endpoint};
use crate::gateway::{router, Caller, Usage};
use crate::models::money::nanos_to_usd;
use crate::models::user::User;
use crate::services::keys;

/// A conversation is resent whole each turn.
const MAX_BODY: usize = 4 * 1024 * 1024;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/api/playground/chat")
            .app_data(web::JsonConfig::default().limit(MAX_BODY))
            .route(web::post().to(chat)),
    )
    .route("/api/playground/models", web::get().to(models))
    .route("/api/playground/cost", web::post().to(cost));
}

/// The hash of a key `user` may use: the keys you may change are the keys
/// you may spend.
async fn key_hash(state: &AppState, user: &User, key_id: i64) -> AppResult<String> {
    keys::authorize(&state.pool, user, key_id).await?;
    Ok(
        sqlx::query_scalar("SELECT key_hash FROM api_keys WHERE id = $1")
            .bind(key_id)
            .fetch_one(&state.pool)
            .await?,
    )
}

/// A chat completions body plus `key_id`, the key to send it with.
async fn chat(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    body: web::Json<Map<String, Value>>,
) -> AppResult<HttpResponse> {
    let mut body = body.into_inner();
    let key_id = body
        .remove("key_id")
        .and_then(|id| id.as_i64())
        .ok_or_else(|| AppError::Validation("`key_id` is required".into()))?;
    let hash = key_hash(&state, &user, key_id).await?;
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("`model` is required".into()))?
        .to_string();

    let gateway = &state.gateway;
    let (key, slot) = gateway.admit_hash(&hash).await?;
    gateway.check_model(&key, &model).await?;
    let caller = Arc::new(Caller {
        key,
        end_user: None,
        tags: vec!["playground".into()],
        slot,
    });
    router::run(gateway, &caller.key, &model, |route| {
        forward::openai_json(
            gateway,
            caller.clone(),
            route,
            Endpoint::ChatCompletions,
            body.clone(),
        )
    })
    .await
}

#[derive(Deserialize)]
struct KeyQuery {
    key_id: i64,
}

/// The model names a key may call, for the Playground's picker.
async fn models(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    query: web::Query<KeyQuery>,
) -> AppResult<HttpResponse> {
    let hash = key_hash(&state, &user, query.key_id).await?;
    let key = state.gateway.authenticate_hash(&hash).await?;
    let names: Vec<String> = state
        .gateway
        .allowed_routes(&key)
        .await?
        .iter()
        .map(|route| route.name.clone())
        .collect();
    Ok(HttpResponse::Ok().json(names))
}

#[derive(Deserialize)]
struct CostRequest {
    /// The deployment that answered: the reply's `x-litellm-model-id`.
    model_id: i64,
    /// OpenAI's `usage`, as the reply carried it.
    usage: Value,
}

/// What a reply cost, priced exactly as the gateway billed it. A stream's
/// cost is only known once it ends, so the Playground asks afterwards.
async fn cost(
    state: web::Data<AppState>,
    _: CurrentUser,
    req: web::Json<CostRequest>,
) -> AppResult<HttpResponse> {
    let routes = state.gateway.routes().await?;
    let route = routes
        .groups
        .values()
        .flatten()
        .find(|route| route.id == req.model_id)
        .ok_or_else(|| AppError::NotFound(format!("model {}", req.model_id)))?;
    let usage = Usage::from_openai(&req.usage).unwrap_or_default();
    Ok(HttpResponse::Ok().json(json!({
        "cost_usd": nanos_to_usd(route.pricing.cost_nanos(&usage)),
    })))
}
