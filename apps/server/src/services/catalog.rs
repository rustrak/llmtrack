//! The price catalog's lifecycle: built-in at first, synced from its
//! URL on an admin's request, the synced copy kept in `settings` so a restart
//! does not fall back to the build's snapshot.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::gateway::catalog::{Catalog, CatalogEntry};
use crate::models::llm_model::PricingUsd;
use crate::services::settings::{self, PRICE_CATALOG, PRICE_CATALOG_SYNCED_AT};

#[derive(Debug, Serialize)]
pub struct CatalogStatus {
    /// `builtin` (the snapshot in this binary) or `synced`.
    pub source: &'static str,
    pub entries: usize,
    pub synced_at: Option<DateTime<Utc>>,
    pub url: String,
}

/// A catalog entry as the dashboard shows it: rates in dollars.
#[derive(Debug, Serialize)]
pub struct EntryView {
    pub key: String,
    /// The id to send the provider (the key without the list's prefix).
    pub model: String,
    pub provider: String,
    pub mode: String,
    pub pricing: PricingUsd,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub supports_reasoning: bool,
    pub supports_prompt_caching: bool,
    pub supports_vision: bool,
    pub supports_function_calling: bool,
}

impl EntryView {
    pub fn new(e: &CatalogEntry, model: String) -> Self {
        Self {
            key: e.key.clone(),
            model,
            provider: e.provider.clone(),
            mode: e.mode.clone(),
            pricing: PricingUsd::from(&e.pricing),
            max_input_tokens: e.max_input_tokens,
            max_output_tokens: e.max_output_tokens,
            supports_reasoning: e.supports_reasoning,
            supports_prompt_caching: e.supports_prompt_caching,
            supports_vision: e.supports_vision,
            supports_function_calling: e.supports_function_calling,
        }
    }
}

pub async fn status(state: &AppState) -> AppResult<CatalogStatus> {
    let synced_at = settings::get(&state.pool, PRICE_CATALOG_SYNCED_AT)
        .await?
        .and_then(|at| DateTime::parse_from_rfc3339(&at).ok())
        .map(|at| at.with_timezone(&Utc));
    Ok(CatalogStatus {
        source: if synced_at.is_some() {
            "synced"
        } else {
            "builtin"
        },
        entries: state.gateway.catalog().len(),
        synced_at,
        url: settings::load(&state.pool).await?.price_catalog_url,
    })
}

/// Loads the synced copy, if there is one, over the built-in snapshot.
pub async fn load_stored(state: &AppState) -> AppResult<()> {
    if let Some(json) = settings::get(&state.pool, PRICE_CATALOG).await? {
        match Catalog::parse(&json) {
            Ok(catalog) => state.gateway.set_catalog(Arc::new(catalog)),
            Err(e) => log::error!("stored price catalog unreadable, using the built-in one: {e}"),
        }
    }
    Ok(())
}

/// Downloads the list, checks it is a price list, stores it and reprices.
/// Any failure leaves the current catalog in place.
pub async fn sync(state: &AppState) -> AppResult<CatalogStatus> {
    let url = settings::load(&state.pool).await?.price_catalog_url;
    let fetch_error =
        |e: String| AppError::Upstream(format!("could not fetch the price list: {e}"));
    let response = reqwest::Client::new()
        .get(&url)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| fetch_error(e.to_string()))?;
    if !response.status().is_success() {
        return Err(fetch_error(format!("{url} answered {}", response.status())));
    }
    let json = response
        .text()
        .await
        .map_err(|e| fetch_error(e.to_string()))?;
    let catalog = Catalog::parse(&json).map_err(fetch_error)?;
    settings::set(&state.pool, PRICE_CATALOG, Some(&json)).await?;
    settings::set(
        &state.pool,
        PRICE_CATALOG_SYNCED_AT,
        Some(&Utc::now().to_rfc3339()),
    )
    .await?;
    state.gateway.set_catalog(Arc::new(catalog));
    status(state).await
}
