//! Wiring shared by `main` and the integration tests, so the tests exercise
//! exactly the routes and middleware that ship.

use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::{Key, SameSite};
use actix_web::web;
use std::time::Duration;

use crate::crypto::SecretBox;
use crate::db::DbPool;
use crate::error::AppError;
use crate::gateway::Gateway;
use crate::routes;

pub struct AppState {
    pub pool: DbPool,
    pub secrets: SecretBox,
    pub gateway: Gateway,
    /// The master key: a bearer token for the management API.
    pub master_key: Option<String>,
}

impl AppState {
    /// `master` is the 64-byte secret; `upstream_timeout` bounds every
    /// provider call.
    pub fn new(pool: DbPool, master: &[u8], upstream_timeout: Duration) -> Self {
        let secrets = SecretBox::new(master);
        Self {
            gateway: Gateway::new(pool.clone(), secrets.clone(), upstream_timeout),
            secrets,
            pool,
            master_key: None,
        }
    }

    pub fn with_master_key(mut self, master_key: Option<String>) -> Self {
        self.master_key = master_key;
        self
    }
}

pub fn session_middleware(key: Key, secure: bool) -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(CookieSessionStore::default(), key)
        .cookie_name("llmtrack_session".into())
        .cookie_secure(secure)
        .cookie_http_only(true)
        .cookie_same_site(SameSite::Lax)
        .build()
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    // Body, path and query errors answer in the same JSON shape as every
    // other error.
    cfg.app_data(
        web::JsonConfig::default()
            .error_handler(|err, _| AppError::Validation(err.to_string()).into()),
    )
    .app_data(
        web::PathConfig::default()
            .error_handler(|err, _| AppError::NotFound(err.to_string()).into()),
    )
    .app_data(
        web::QueryConfig::default()
            .error_handler(|err, _| AppError::Validation(err.to_string()).into()),
    )
    .configure(routes::health::configure)
    .configure(routes::auth::configure)
    .configure(routes::users::configure)
    .configure(routes::invitations::configure)
    .configure(routes::models::configure)
    .configure(routes::teams::configure)
    .configure(routes::people::configure)
    .configure(routes::playground::configure)
    .configure(routes::keys::configure)
    .configure(routes::labels::configure)
    .configure(routes::usage::configure)
    .configure(routes::settings::configure)
    .configure(routes::proxy::configure);
}
