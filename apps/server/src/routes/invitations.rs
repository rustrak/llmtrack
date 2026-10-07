use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::AdminUser;
use crate::error::AppResult;
use crate::models::invitation::CreateInvitationRequest;
use crate::services::invitations;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/invitations")
            .default_service(web::to(super::not_found))
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{token}", web::delete().to(revoke)),
    );
}

async fn list(state: web::Data<AppState>, _: AdminUser) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(invitations::list(&state.pool).await?))
}

async fn create(
    state: web::Data<AppState>,
    AdminUser(user): AdminUser,
    req: web::Json<CreateInvitationRequest>,
) -> AppResult<HttpResponse> {
    let invitation = invitations::create(&state.pool, &req.email, &req.role, user.id).await?;
    Ok(HttpResponse::Created().json(invitation))
}

async fn revoke(
    state: web::Data<AppState>,
    _: AdminUser,
    token: web::Path<String>,
) -> AppResult<HttpResponse> {
    invitations::revoke(&state.pool, &token).await?;
    Ok(HttpResponse::NoContent().finish())
}
