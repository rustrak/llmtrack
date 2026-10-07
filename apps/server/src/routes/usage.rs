use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::AppResult;
use crate::models::list::ListQuery;
use crate::services::usage::{self, LogQuery, UsageQuery};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/usage", web::get().to(report))
        .route("/api/logs", web::get().to(logs));
}

async fn report(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    query: web::Query<UsageQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(usage::report(&state.pool, &user, &query).await?))
}

async fn logs(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    list: web::Query<ListQuery>,
    query: web::Query<LogQuery>,
) -> AppResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(usage::logs(&state.pool, &user, &list, &query).await?))
}
