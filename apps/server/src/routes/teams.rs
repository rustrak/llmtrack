use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::{AdminUser, CurrentUser};
use crate::error::AppResult;
use crate::models::list::ListQuery;
use crate::models::team::{
    AddMemberRequest, CreateTeamRequest, TeamListQuery, UpdateMemberRequest, UpdateTeamRequest,
};
use crate::services::access::{require_team_manager, team_access};
use crate::services::teams;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/teams")
            .default_service(web::to(super::not_found))
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}", web::get().to(get))
            .route("/{id}", web::patch().to(update))
            .route("/{id}", web::delete().to(delete))
            .route("/{id}/members", web::post().to(add_member))
            .route("/{id}/members/{user_id}", web::patch().to(update_member))
            .route("/{id}/members/{user_id}", web::delete().to(remove_member)),
    );
}

async fn list(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    list: web::Query<ListQuery>,
    filter: web::Query<TeamListQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(teams::list_for(&state.pool, &user, &list, &filter).await?))
}

async fn get(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    let id = id.into_inner();
    team_access(&state.pool, &user, id).await?;
    Ok(HttpResponse::Ok().json(teams::detail(&state.pool, &user, id).await?))
}

async fn create(
    state: web::Data<AppState>,
    AdminUser(user): AdminUser,
    req: web::Json<CreateTeamRequest>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Created().json(teams::create(&state.pool, &user, &req).await?))
}

async fn update(
    state: web::Data<AppState>,
    AdminUser(user): AdminUser,
    id: web::Path<i64>,
    req: web::Json<UpdateTeamRequest>,
) -> AppResult<HttpResponse> {
    let id = id.into_inner();
    team_access(&state.pool, &user, id).await?;
    let team = teams::update(&state.pool, &user, id, &req).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::Ok().json(team))
}

async fn delete(
    state: web::Data<AppState>,
    _: AdminUser,
    id: web::Path<i64>,
) -> AppResult<HttpResponse> {
    teams::delete(&state.pool, id.into_inner()).await?;
    state.gateway.invalidate();
    Ok(HttpResponse::NoContent().finish())
}

async fn add_member(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    id: web::Path<i64>,
    req: web::Json<AddMemberRequest>,
) -> AppResult<HttpResponse> {
    let id = id.into_inner();
    require_team_manager(&state.pool, &user, id).await?;
    let member = teams::add_member(&state.pool, id, &req.email, &req.role).await?;
    Ok(HttpResponse::Created().json(member))
}

async fn update_member(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    path: web::Path<(i64, i64)>,
    req: web::Json<UpdateMemberRequest>,
) -> AppResult<HttpResponse> {
    let (id, user_id) = path.into_inner();
    require_team_manager(&state.pool, &user, id).await?;
    Ok(HttpResponse::Ok().json(teams::update_member(&state.pool, id, user_id, &req.role).await?))
}

async fn remove_member(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    path: web::Path<(i64, i64)>,
) -> AppResult<HttpResponse> {
    let (id, user_id) = path.into_inner();
    require_team_manager(&state.pool, &user, id).await?;
    teams::remove_member(&state.pool, id, user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}
