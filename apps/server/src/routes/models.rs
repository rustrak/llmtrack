use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::{AdminUser, CurrentUser};
use crate::error::AppResult;
use crate::gateway::forward;
use crate::gateway::providers::PROVIDERS;
use crate::models::list::ListQuery;
use crate::models::llm_model::{
    CreateModelRequest, ModelListItem, ModelListQuery, TestModelRequest, UpdateModelRequest,
};
use crate::services::models;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/providers", web::get().to(list_providers))
        .service(
            web::scope("/api/models")
                .default_service(web::to(super::not_found))
                .route("", web::get().to(list))
                .route("", web::post().to(create))
                .route("/test", web::post().to(test_connection))
                .route("/{id}", web::patch().to(update))
                .route("/{id}", web::delete().to(delete)),
        );
}

async fn list_providers(_: CurrentUser) -> HttpResponse {
    HttpResponse::Ok().json(PROVIDERS)
}

async fn list(
    state: web::Data<AppState>,
    _: CurrentUser,
    list: web::Query<ListQuery>,
    filter: web::Query<ModelListQuery>,
) -> AppResult<HttpResponse> {
    let catalog = state.gateway.catalog();
    let page = models::page(&state.pool, &list, &filter).await?;
    let items = page
        .data
        .into_iter()
        .map(|row| ModelListItem {
            model: models::to_response(row.model, &catalog),
            deployments: row.deployments,
        })
        .collect();
    Ok(HttpResponse::Ok().json(list.paged(items, page.total)))
}

async fn create(
    state: web::Data<AppState>,
    _: AdminUser,
    req: web::Json<CreateModelRequest>,
) -> AppResult<HttpResponse> {
    let catalog = state.gateway.catalog();
    let model = models::create(&state.pool, &state.secrets, &catalog, &req).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Created().json(models::to_response(model, &catalog)))
}

async fn update(
    state: web::Data<AppState>,
    _: AdminUser,
    id: web::Path<i64>,
    req: web::Json<UpdateModelRequest>,
) -> AppResult<HttpResponse> {
    let catalog = state.gateway.catalog();
    let model =
        models::update(&state.pool, &state.secrets, &catalog, id.into_inner(), &req).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Ok().json(models::to_response(model, &catalog)))
}

async fn delete(
    state: web::Data<AppState>,
    _: AdminUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    models::delete(&state.pool, id.into_inner()).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::NoContent().finish())
}

async fn test_connection(
    state: web::Data<AppState>,
    _: AdminUser,
    req: web::Json<TestModelRequest>,
) -> AppResult<HttpResponse> {
    let catalog = state.gateway.catalog();
    let (route, embedding) =
        models::probe_route(&state.pool, &state.secrets, &catalog, &req).await?;
    Ok(HttpResponse::Ok().json(forward::probe(&state.gateway, &route, embedding).await))
}
