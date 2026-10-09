use actix_web::http::header::{ACCEPT_LANGUAGE, CONTENT_DISPOSITION};
use actix_web::{web, HttpRequest, HttpResponse};

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::AppResult;
use crate::models::list::ListQuery;
use crate::services::report::{self, ExportQuery};
use crate::services::usage::{self, LogQuery, UsageQuery};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/usage", web::get().to(report))
        .route("/api/usage/export", web::get().to(export))
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

/// The report as a file to download: PDF or XLSX, in the reader's language
/// and currency.
async fn export(
    state: web::Data<AppState>,
    CurrentUser(user): CurrentUser,
    req: HttpRequest,
    query: web::Query<ExportQuery>,
) -> AppResult<HttpResponse> {
    let accept_language = req
        .headers()
        .get(ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok());
    let file = report::export(&state, &user, &query, accept_language).await?;
    Ok(HttpResponse::Ok()
        .content_type(file.content_type)
        .insert_header((
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file.name),
        ))
        .body(file.bytes))
}
