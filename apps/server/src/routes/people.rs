use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::AppResult;
use crate::models::list::ListQuery;
use crate::models::person::{CreatePersonRequest, PersonListQuery, UpdatePersonRequest};
use crate::services::people;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/people")
            .default_service(web::to(super::not_found))
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}", web::patch().to(update))
            .route("/{id}", web::delete().to(delete)),
    );
}

async fn list(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    list: web::Query<ListQuery>,
    filter: web::Query<PersonListQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(people::list_for(&state.pool, &user, &list, &filter).await?))
}

async fn create(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    req: web::Json<CreatePersonRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Created().json(people::create(&state.pool, &user, &req).await?))
}

async fn update(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
    req: web::Json<UpdatePersonRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(people::update(&state.pool, &user, id.into_inner(), &req).await?))
}

async fn delete(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    people::delete(&state.pool, &user, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}
