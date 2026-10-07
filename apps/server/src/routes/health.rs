use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::app::AppState;
use crate::db;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(liveness))
        .route("/health/ready", web::get().to(readiness));
}

/// The process answers.
async fn liveness() -> HttpResponse {
    HttpResponse::Ok().json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// The process can serve: the database answers too.
async fn readiness(state: web::Data<AppState>) -> HttpResponse {
    if db::health_check(&state.pool).await {
        HttpResponse::Ok().json(json!({ "status": "ok", "database": "ok" }))
    } else {
        HttpResponse::ServiceUnavailable()
            .json(json!({ "status": "unavailable", "database": "down" }))
    }
}
