pub mod auth;
pub mod dashboard;
pub mod health;
pub mod invitations;
pub mod keys;
pub mod labels;
pub mod models;
pub mod people;
pub mod playground;
pub mod proxy;
pub mod teams;
pub mod usage;
pub mod users;

use actix_web::{HttpRequest, HttpResponse, ResponseError};

/// The JSON 404 for a path no handler claimed. Every scope ends in it, or an
/// unknown `/api/teams/x/y` would answer an empty 404 instead of an error body.
pub async fn not_found(req: HttpRequest) -> HttpResponse {
    crate::error::AppError::NotFound(req.path().to_string()).error_response()
}
pub mod settings;
