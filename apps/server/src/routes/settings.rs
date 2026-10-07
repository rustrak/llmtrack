use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::app::AppState;
use crate::auth::{AdminUser, CurrentUser};
use crate::error::AppResult;
use crate::services::catalog::{self, EntryView};
use crate::services::settings::{self, UpdateSettingsRequest};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/settings", web::get().to(get))
        .route("/api/settings", web::patch().to(update))
        .route("/api/catalog", web::get().to(search))
        .route("/api/catalog/status", web::get().to(status))
        .route("/api/catalog/sync", web::post().to(sync))
        .route("/api/router-settings", web::get().to(get_router))
        .route("/api/router-settings", web::put().to(set_router));
}

/// Everyone reads the public URL (for code samples); admins read it all.
async fn get(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<HttpResponse> {
    let settings = settings::load(&state.pool).await?;
    if user.is_admin() {
        return Ok(HttpResponse::Ok().json(settings));
    }
    Ok(HttpResponse::Ok().json(json!({ "public_url": settings.public_url })))
}

/// The router settings: retries, cooldowns, fallbacks.
async fn get_router(state: web::Data<AppState>, _: AdminUser) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(settings::router(&state.pool).await?))
}

async fn set_router(
    state: web::Data<AppState>,
    _: AdminUser,
    req: web::Json<crate::gateway::router::RouterSettings>,
) -> AppResult<HttpResponse> {
    let saved = settings::set_router(&state.pool, &req).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Ok().json(saved))
}

async fn update(
    state: web::Data<AppState>,
    _: AdminUser,
    req: web::Json<UpdateSettingsRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(settings::update(&state.pool, &req).await?))
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    /// One of our provider ids: lists that provider's models.
    provider: Option<String>,
    limit: Option<usize>,
}

async fn search(
    state: web::Data<AppState>,
    _: CurrentUser,
    query: web::Query<SearchQuery>,
) -> AppResult<HttpResponse> {
    let catalog = state.gateway.catalog();
    let limit = query.limit.unwrap_or(20).clamp(1, 1000);
    let hits: Vec<EntryView> = match &query.provider {
        Some(provider) => catalog
            .models_for(provider, &query.q, limit)
            .into_iter()
            .map(|(e, model)| EntryView::new(e, model))
            .collect(),
        None => catalog
            .search(&query.q, limit)
            .into_iter()
            .map(|e| EntryView::new(e, e.key.clone()))
            .collect(),
    };
    Ok(HttpResponse::Ok().json(hits))
}

async fn status(state: web::Data<AppState>, _: CurrentUser) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(catalog::status(&state).await?))
}

async fn sync(state: web::Data<AppState>, _: AdminUser) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(catalog::sync(&state).await?))
}
