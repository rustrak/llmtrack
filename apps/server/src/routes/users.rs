use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::AdminUser;
use crate::error::AppResult;
use crate::models::list::ListQuery;
use crate::models::user::{CreateUserRequest, UpdateUserRequest};
use crate::services::users;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/users")
            .default_service(web::to(super::not_found))
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}", web::patch().to(update))
            .route("/{id}", web::delete().to(delete)),
    );
}

async fn list(
    state: web::Data<AppState>,
    _: AdminUser,
    list: web::Query<ListQuery>,
    filter: web::Query<users::UserListQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(users::list(&state.pool, &list, &filter).await?))
}

async fn create(
    state: web::Data<AppState>,
    _: AdminUser,
    req: web::Json<CreateUserRequest>,
) -> AppResult<HttpResponse> {
    let user = users::create(&state.pool, &req.email, &req.password, &req.role).await?;
    Ok(HttpResponse::Created().json(user))
}

async fn update(
    state: web::Data<AppState>,
    AdminUser(actor): AdminUser,
    id: web::Path<i64>,
    req: web::Json<UpdateUserRequest>,
) -> AppResult<HttpResponse> {
    let user = users::update(&state.pool, actor.id, id.into_inner(), &req).await?;
    Ok(HttpResponse::Ok().json(user))
}

async fn delete(
    state: web::Data<AppState>,
    AdminUser(actor): AdminUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    users::delete(&state.pool, actor.id, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}
