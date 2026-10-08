use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::{AdminUser, CurrentUser};
use crate::error::AppResult;
use crate::models::label::{CreateLabelRequest, UpdateLabelRequest};
use crate::models::list::ListQuery;
use crate::services::labels;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/labels")
            .default_service(web::to(super::not_found))
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}", web::patch().to(update))
            .route("/{id}", web::delete().to(delete)),
    );
}

/// Anyone signed in: keys and usage filters pick from them.
async fn list(
    state: web::Data<AppState>,
    _: CurrentUser,
    list: web::Query<ListQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(labels::list(&state.pool, &list).await?))
}

async fn create(
    state: web::Data<AppState>,
    _: AdminUser,
    req: web::Json<CreateLabelRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Created().json(labels::create(&state.pool, &req).await?))
}

async fn update(
    state: web::Data<AppState>,
    _: AdminUser,
    id: web::Path<i64>,
    req: web::Json<UpdateLabelRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(labels::update(&state.pool, id.into_inner(), &req).await?))
}

async fn delete(
    state: web::Data<AppState>,
    _: AdminUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    labels::delete(&state.pool, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}
