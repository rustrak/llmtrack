use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::app::AppState;
use crate::auth::{start_session, CurrentUser};
use crate::error::{AppError, AppResult};
use crate::models::invitation::{AcceptInvitationRequest, InvitationInfo};
use crate::models::user::{
    validate_password, ChangePasswordRequest, LoginRequest, UpdateProfileRequest,
};
use crate::services::{invitations, users};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .default_service(web::to(super::not_found))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/me", web::get().to(me))
            .route("/me", web::patch().to(update_me))
            .route("/me/password", web::post().to(change_password))
            .route("/invitation/{token}", web::get().to(invitation))
            .route("/accept-invitation", web::post().to(accept_invitation)),
    );
}

async fn login(
    state: web::Data<AppState>,
    session: Session,
    req: web::Json<LoginRequest>,
) -> AppResult<HttpResponse> {
    // One message for every failure, so the form cannot be used to find out
    // which emails have accounts.
    let invalid = || AppError::Unauthorized("invalid email or password".into());
    let user = users::get_by_email(&state.pool, &req.email)
        .await?
        .ok_or_else(invalid)?;
    if !user.is_active || !user.verify_password(&req.password)? {
        return Err(invalid());
    }
    users::touch_last_login(&state.pool, user.id).await?;
    start_session(&session, user.id)?;
    let user = users::require(&state.pool, user.id).await?;
    Ok(HttpResponse::Ok().json(json!({ "user": user })))
}

async fn logout(session: Session) -> HttpResponse {
    session.purge();
    HttpResponse::NoContent().finish()
}

async fn me(CurrentUser(user): CurrentUser) -> HttpResponse {
    HttpResponse::Ok().json(json!({ "user": user }))
}

async fn update_me(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    req: web::Json<UpdateProfileRequest>,
) -> AppResult<HttpResponse> {
    let user = users::update_profile(&state.pool, user.id, &req).await?;
    Ok(HttpResponse::Ok().json(json!({ "user": user })))
}

async fn change_password(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    req: web::Json<ChangePasswordRequest>,
) -> AppResult<HttpResponse> {
    if !user.verify_password(&req.current_password)? {
        return Err(AppError::Unauthorized("current password is wrong".into()));
    }
    validate_password(&req.new_password, "new_password")?;
    users::set_password(&state.pool, user.id, &req.new_password).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// What the public invite page shows before the password is set.
async fn invitation(
    state: web::Data<AppState>,
    token: web::Path<String>,
) -> AppResult<HttpResponse> {
    let invitation = invitations::acceptable(&state.pool, &token).await?;
    Ok(HttpResponse::Ok().json(InvitationInfo {
        email: invitation.email,
        role: invitation.role,
        expires_at: invitation.expires_at,
    }))
}

async fn accept_invitation(
    state: web::Data<AppState>,
    session: Session,
    req: web::Json<AcceptInvitationRequest>,
) -> AppResult<HttpResponse> {
    let user = invitations::accept(&state.pool, &req.token, &req.password).await?;
    start_session(&session, user.id)?;
    Ok(HttpResponse::Created().json(json!({ "user": user })))
}
