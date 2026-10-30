//! The MCP server at `/mcp`, and the instance switch that opens it.
//!
//! Stateless Streamable HTTP: every call is one `POST` with nothing kept
//! between them, so the endpoint costs nothing while idle and needs no session
//! store. `rmcp` speaks the protocol; this module adapts Actix to its `http`
//! 1.x types and gives each request a handler that already knows who is
//! calling. The tools themselves live in `crate::mcp`.
//!
//! Callers sign in with OAuth (`routes::oauth`): an access token issued for
//! this endpoint is the only credential it takes. Not the dashboard cookie,
//! so a browser tab cannot call tools on its user's behalf, and not `sk-`
//! keys, which are for the gateway.

use std::convert::Infallible;
use std::sync::Arc;

use actix_web::http::header::{HeaderName, HeaderValue, AUTHORIZATION, HOST, WWW_AUTHENTICATE};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use http_body_util::{BodyStream, Full};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::auth::{AdminUser, CurrentUser};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::mcp::LlmtrackMcp;
use crate::models::user::User;
use crate::routes::oauth;
use crate::services::oauth::{OAuthService, ACCESS_PREFIX};
use crate::services::{settings, users};

/// Turns `/mcp` on or off: the body of `PUT /api/settings/mcp`.
#[derive(Debug, Deserialize)]
pub struct McpSettings {
    pub enabled: bool,
}

/// Whether `/mcp` answers, and the URL to give an assistant: the public URL
/// plus `/mcp`, which OAuth checks the resource against. `null` until an
/// admin sets the public URL, when signing in is not possible.
#[derive(Debug, Serialize)]
pub struct McpStatus {
    pub enabled: bool,
    pub url: Option<String>,
}

impl McpStatus {
    async fn new(pool: &DbPool, enabled: bool) -> AppResult<Self> {
        Ok(Self {
            enabled,
            url: oauth::issuer(pool)
                .await?
                .map(|base| oauth::mcp_resource(&base)),
        })
    }
}

async fn is_enabled(pool: &DbPool) -> AppResult<bool> {
    Ok(settings::get(pool, settings::MCP_ENABLED).await?.as_deref() == Some("true"))
}

/// Whether the MCP endpoint at `/mcp` is on.
async fn get_settings(state: web::Data<AppState>, _: CurrentUser) -> AppResult<HttpResponse> {
    let enabled = is_enabled(&state.pool).await?;
    Ok(HttpResponse::Ok().json(McpStatus::new(&state.pool, enabled).await?))
}

/// Turns the MCP endpoint at `/mcp` on or off. Admins only.
async fn update_settings(
    state: web::Data<AppState>,
    _: AdminUser,
    body: web::Json<McpSettings>,
) -> AppResult<HttpResponse> {
    let value = body.enabled.then_some("true");
    settings::set(&state.pool, settings::MCP_ENABLED, value).await?;
    Ok(HttpResponse::Ok().json(McpStatus::new(&state.pool, body.enabled).await?))
}

/// Who is calling `/mcp`: the active user behind a current OAuth access token
/// issued for it. `None` when there is none.
async fn caller(
    req: &HttpRequest,
    pool: &DbPool,
    resource: Option<&str>,
) -> AppResult<Option<User>> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| t.starts_with(ACCESS_PREFIX));
    // Without an issuer no OAuth token was ever valid here.
    let (Some(token), Some(resource)) = (token, resource) else {
        return Ok(None);
    };
    let Some(user_id) = OAuthService::authenticate(pool, token, resource).await? else {
        return Ok(None);
    };
    Ok(users::get(pool, user_id).await?.filter(|u| u.is_active))
}

/// `/mcp`: authenticate, then hand the request to `rmcp`.
async fn serve(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> AppResult<HttpResponse> {
    if !is_enabled(&state.pool).await? {
        return Err(AppError::NotFound("/mcp".to_string()));
    }
    let base = oauth::issuer(&state.pool).await?;
    let resource = base.as_deref().map(oauth::mcp_resource);
    let Some(user) = caller(&req, &state.pool, resource.as_deref()).await? else {
        // RFC 9728: tells an OAuth client where to start authorizing, when
        // there is an issuer to send it to.
        let challenge = match base.as_deref() {
            Some(base) => format!(
                "Bearer resource_metadata=\"{}\"",
                oauth::resource_metadata_url(base)
            ),
            None => "Bearer".to_string(),
        };
        return Ok(HttpResponse::Unauthorized()
            .insert_header((WWW_AUTHENTICATE, challenge))
            .json(serde_json::json!({
                "error": { "type": "unauthorized", "message": "a valid bearer token is required" }
            })));
    };
    let handler = LlmtrackMcp::new(state, user);

    let mut request = http::Request::builder()
        .method(req.method().as_str())
        .uri(req.uri().to_string());
    for (name, value) in req.headers() {
        request = request.header(name.as_str(), value.as_bytes());
    }
    // rmcp rejects a request without `Host` (an HTTP/1.0 client may send
    // none). It reads the value only for the DNS-rebinding check, which is
    // disabled below, so any name satisfies it.
    if !req.headers().contains_key(HOST) {
        request = request.header("host", "localhost");
    }
    let request = request
        .body(Full::new(body))
        .map_err(|e| AppError::Internal(format!("MCP request: {e}")))?;

    // ponytail: one service per request so the handler owns its caller; a
    // shared service would need the user smuggled through request extensions.
    let service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(NeverSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            // Host checks guard unauthenticated local servers from DNS
            // rebinding. Every call here carries a token a rebinding page
            // cannot read, and the public hostname is the operator's.
            .disable_allowed_hosts(),
    );
    let (parts, body) = service.handle(request).await.into_parts();

    let mut response = HttpResponse::build(
        StatusCode::from_u16(parts.status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
    );
    for (name, value) in &parts.headers {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            response.insert_header((name, value));
        }
    }
    let body = BodyStream::new(body)
        .filter_map(|frame| async move { frame.ok()?.into_data().ok().map(Ok::<_, Infallible>) });
    Ok(response.streaming(body))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    // Every method: rmcp answers what it does not serve (a legacy client's
    // `GET` stream, `DELETE`) with the 405 the transport spec asks for.
    cfg.route("/mcp", web::route().to(serve)).service(
        web::resource("/api/settings/mcp")
            .route(web::get().to(get_settings))
            .route(web::put().to(update_settings)),
    );
}
