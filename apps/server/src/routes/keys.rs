use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::AppResult;
use crate::models::key::{CreateKeyRequest, KeyListQuery, UpdateKeyRequest};
use crate::models::list::ListQuery;
use crate::services::keys;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/keys")
            .default_service(web::to(super::not_found))
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}", web::get().to(detail))
            .route("/{id}", web::patch().to(update))
            .route(
                "/{id}/block",
                web::post().to(|s, u, id| set_blocked(s, u, id, true)),
            )
            .route(
                "/{id}/unblock",
                web::post().to(|s, u, id| set_blocked(s, u, id, false)),
            )
            .route("/{id}/regenerate", web::post().to(regenerate))
            .route("/{id}", web::delete().to(revoke)),
    );
}

async fn list(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    list: web::Query<ListQuery>,
    filter: web::Query<KeyListQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(keys::list_for(&state.pool, &user, &list, &filter).await?))
}

async fn create(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    req: web::Json<CreateKeyRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Created().json(keys::create(&state.pool, &user, &req).await?))
}

async fn update(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
    req: web::Json<UpdateKeyRequest>,
) -> AppResult<HttpResponse> {
    let key = keys::update(&state.pool, &user, id.into_inner(), &req).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Ok().json(key))
}

async fn revoke(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    keys::revoke(&state.pool, &user, id.into_inner()).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::NoContent().finish())
}

async fn detail(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(keys::detail(&state.pool, &user, id.into_inner()).await?))
}

async fn set_blocked(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
    blocked: bool,
) -> AppResult<HttpResponse> {
    let key = keys::set_blocked(&state.pool, &user, id.into_inner(), blocked).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Ok().json(key))
}

async fn regenerate(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    let key = keys::regenerate(&state.pool, &user, id.into_inner()).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Ok().json(key))
}
