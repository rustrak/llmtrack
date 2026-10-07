//! Instance-wide settings, one row per key.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::gateway::catalog;
use crate::gateway::router::RouterSettings;
use crate::models::patch;

pub const PUBLIC_URL: &str = "public_url";
pub const PRICE_CATALOG_URL: &str = "price_catalog_url";
/// The price list's JSON as last synced; absent until the first sync.
pub const PRICE_CATALOG: &str = "price_catalog";
pub const PRICE_CATALOG_SYNCED_AT: &str = "price_catalog_synced_at";

#[derive(Debug, Serialize)]
pub struct GlobalSettings {
    /// Where applications reach the gateway; the dashboard's code samples use it.
    pub public_url: Option<String>,
    pub price_catalog_url: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    #[serde(default, deserialize_with = "patch")]
    pub public_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch")]
    pub price_catalog_url: Option<Option<String>>,
}

pub async fn get(pool: &DbPool, key: &str) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT value FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await?,
    )
}

/// Writes `value`, or deletes the key for `None`.
pub async fn set(pool: &DbPool, key: &str, value: Option<&str>) -> AppResult<()> {
    match value {
        Some(value) => {
            sqlx::query(
                "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, $3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value)
            .bind(Utc::now())
            .execute(pool)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM settings WHERE key = $1")
                .bind(key)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn load(pool: &DbPool) -> AppResult<GlobalSettings> {
    Ok(GlobalSettings {
        public_url: get(pool, PUBLIC_URL).await?,
        price_catalog_url: get(pool, PRICE_CATALOG_URL)
            .await?
            .unwrap_or_else(|| catalog::DEFAULT_URL.to_string()),
    })
}

/// An http(s) URL without a trailing slash.
fn http_url(value: &str, field: &str) -> AppResult<String> {
    let value = value.trim();
    let valid = reqwest::Url::parse(value)
        .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some());
    if !valid {
        return Err(
            AppError::Validation(format!("'{value}' is not an http(s) URL"))
                .with_field(field, FieldErrorCode::Invalid),
        );
    }
    Ok(value.trim_end_matches('/').to_string())
}

pub async fn update(pool: &DbPool, req: &UpdateSettingsRequest) -> AppResult<GlobalSettings> {
    if let Some(url) = &req.public_url {
        let url = url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .map(|u| http_url(u, PUBLIC_URL))
            .transpose()?;
        set(pool, PUBLIC_URL, url.as_deref()).await?;
    }
    if let Some(url) = &req.price_catalog_url {
        let url = url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .map(|u| http_url(u, PRICE_CATALOG_URL))
            .transpose()?;
        set(pool, PRICE_CATALOG_URL, url.as_deref()).await?;
    }
    load(pool).await
}

/// The router settings, JSON; absent means the defaults.
pub const ROUTER: &str = "router_settings";

pub async fn router(pool: &DbPool) -> AppResult<RouterSettings> {
    Ok(get(pool, ROUTER)
        .await?
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

/// Replaces the router settings: anything left out takes its default.
/// Fallbacks must name models that exist.
pub async fn set_router(pool: &DbPool, settings: &RouterSettings) -> AppResult<RouterSettings> {
    for (field, value, max) in [
        ("num_retries", settings.num_retries, 10),
        ("allowed_fails", settings.allowed_fails, 1000),
        ("cooldown_time", settings.cooldown_time, 3600),
    ] {
        if !(0..=max).contains(&value) {
            return Err(
                AppError::Validation(format!("{field} must be between 0 and {max}"))
                    .with_field(field, FieldErrorCode::Invalid),
            );
        }
    }
    let names: std::collections::HashSet<String> =
        sqlx::query_scalar("SELECT DISTINCT name FROM models")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    for (field, list) in [
        ("fallbacks", &settings.fallbacks),
        (
            "context_window_fallbacks",
            &settings.context_window_fallbacks,
        ),
        (
            "content_policy_fallbacks",
            &settings.content_policy_fallbacks,
        ),
    ] {
        for entry in list {
            let unknown = (entry.len() != 1)
                .then(|| "each entry maps one model to its fallbacks".to_string())
                .or_else(|| {
                    entry
                        .iter()
                        .flat_map(|(from, to)| {
                            std::iter::once(from).filter(|f| *f != "*").chain(to)
                        })
                        .find(|name| !names.contains(*name))
                        .map(|name| format!("no model is named '{name}'"))
                });
            if let Some(message) = unknown {
                return Err(AppError::Validation(format!("{field}: {message}"))
                    .with_field(field, FieldErrorCode::Invalid));
            }
        }
    }
    let json = serde_json::to_string(settings).map_err(|e| AppError::Internal(e.to_string()))?;
    set(pool, ROUTER, Some(&json)).await?;
    Ok(settings.clone())
}
