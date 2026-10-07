//! Who is calling the management API (`/api`, `/auth/me`).
//!
//! The dashboard sends an encrypted session cookie naming the user. Scripts
//! send `Authorization: Bearer`: the
//! master key (`MASTER_KEY`) acts as the first active admin, and a personal
//! virtual key acts as its owner, with the owner's rights. Team keys bill a
//! team, not a person, so they cannot manage anything.

use actix_session::{Session, SessionExt};
use actix_web::{dev::Payload, web, FromRequest, HttpRequest};
use std::future::Future;
use std::pin::Pin;

use crate::app::AppState;
use crate::crypto::hash_key;
use crate::error::{AppError, AppResult};
use crate::models::user::User;
use crate::services::users;

const SESSION_USER_ID: &str = "user_id";

pub fn start_session(session: &Session, user_id: i64) -> AppResult<()> {
    // A new session id on every login: a cookie planted before login is never
    // promoted to an authenticated one.
    session.renew();
    session
        .insert(SESSION_USER_ID, user_id)
        .map_err(|e| AppError::Internal(format!("failed to write session: {e}")))
}

/// The signed-in, active user. Re-read on every request, so disabling an
/// account ends its sessions at once.
pub struct CurrentUser(pub User);

impl FromRequest for CurrentUser {
    type Error = AppError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let bearer = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.strip_prefix("Bearer ")
                    .or_else(|| v.strip_prefix("bearer "))
            })
            .map(|v| v.trim().to_string());
        let user_id = req.get_session().get::<i64>(SESSION_USER_ID).ok().flatten();
        let state = req.app_data::<web::Data<AppState>>().cloned();

        Box::pin(async move {
            let unauthenticated = || AppError::Unauthorized("not authenticated".into());
            let state = state.ok_or_else(|| AppError::Internal("AppState missing".into()))?;
            let user_id = match bearer {
                Some(token) => bearer_user(&state, &token).await?,
                None => user_id.ok_or_else(unauthenticated)?,
            };
            match users::get(&state.pool, user_id).await? {
                Some(user) if user.is_active => Ok(CurrentUser(user)),
                _ => Err(unauthenticated()),
            }
        })
    }
}

/// A [`CurrentUser`] who is a global admin.
pub struct AdminUser(pub User);

impl FromRequest for AdminUser {
    type Error = AppError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let current = CurrentUser::from_request(req, payload);
        Box::pin(async move {
            let CurrentUser(user) = current.await?;
            if !user.is_admin() {
                return Err(AppError::Forbidden("admin only".into()));
            }
            Ok(AdminUser(user))
        })
    }
}

/// The user a bearer token acts as.
async fn bearer_user(state: &AppState, token: &str) -> AppResult<i64> {
    let is_master = state
        .master_key
        .as_deref()
        .is_some_and(|master| hash_key(master) == hash_key(token));
    if is_master {
        // ponytail: the master key borrows the oldest admin's identity, so
        // what it creates has an author; a service account if that matters.
        return users::first_admin(&state.pool)
            .await?
            .ok_or_else(|| AppError::Unauthorized("there is no active admin to act as".into()));
    }
    let key = state
        .gateway
        .authenticate(token)
        .await
        .map_err(|_| AppError::Unauthorized("invalid API key".into()))?;
    key.usable(chrono::Utc::now())?;
    key.user_id.ok_or_else(|| {
        AppError::Forbidden(
            "team keys cannot call the management API; use a personal key or the master key".into(),
        )
    })
}
