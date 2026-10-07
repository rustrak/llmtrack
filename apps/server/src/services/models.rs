use chrono::Utc;

use crate::crypto::SecretBox;
use crate::db::DbPool;
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::gateway::catalog::Catalog;
use crate::gateway::pricing::Pricing;
use crate::gateway::providers;
use crate::models::list::{ListQuery, Paged};
use crate::models::llm_model::{
    CreateModelRequest, LlmModel, ModelListQuery, ModelResponse, PricingSource, PricingUsd,
    TestModelRequest, UpdateModelRequest, PRICING_MODES,
};
use crate::models::required_name;

macro_rules! columns {
    () => {
        "id, name, provider, upstream_model, api_base, api_version, api_key_encrypted, \
         catalog_key, pricing, is_active, created_at, weight, pricing_mode"
    };
}

/// A model after validation, ready to write.
struct Fields {
    name: String,
    provider: String,
    upstream_model: String,
    api_base: Option<String>,
    api_version: Option<String>,
    api_key_encrypted: Option<String>,
    catalog_key: Option<String>,
    pricing: Option<String>,
    is_active: bool,
    weight: Option<i64>,
    pricing_mode: String,
}

pub async fn list(pool: &DbPool) -> AppResult<Vec<LlmModel>> {
    Ok(sqlx::query_as::<_, LlmModel>(concat!(
        "SELECT ",
        columns!(),
        " FROM models ORDER BY name, id"
    ))
    .fetch_all(pool)
    .await?)
}

#[derive(sqlx::FromRow)]
pub struct ModelPageRow {
    #[sqlx(flatten)]
    pub model: LlmModel,
    /// How many models share this one's name: its deployments.
    pub deployments: i64,
}

/// The models table, a page at a time, each with its name's deployment count.
pub async fn page(
    pool: &DbPool,
    list: &ListQuery,
    filter: &ModelListQuery,
) -> AppResult<Paged<ModelPageRow>> {
    let active = match filter.status.as_deref() {
        None | Some("") => None,
        Some("active") => Some(true),
        Some("inactive") => Some(false),
        Some(other) => {
            return Err(AppError::Validation(format!(
                "status must be 'active' or 'inactive', not '{other}'"
            ))
            .with_field("status", FieldErrorCode::Invalid))
        }
    };
    let order = list.order_by(
        &[
            ("name", "LOWER(m.name)"),
            ("provider", "m.provider"),
            ("upstream_model", "LOWER(m.upstream_model)"),
            ("created_at", "m.created_at"),
        ],
        "name",
        "m.id",
    )?;
    const WHERE: &str = "
        WHERE ($1 IS NULL OR m.provider = $1)
          AND ($2 IS NULL OR m.is_active = $2)
          AND ($3 IS NULL OR LOWER(m.name) LIKE $3 ESCAPE '\\'
               OR LOWER(m.upstream_model) LIKE $3 ESCAPE '\\')";
    let provider = filter.provider.as_deref().filter(|p| !p.is_empty());
    let search = list.search();
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM models m {WHERE}"
    )))
    .bind(provider)
    .bind(active)
    .bind(&search)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query_as::<_, ModelPageRow>(sqlx::AssertSqlSafe(format!(
        "SELECT m.id, m.name, m.provider, m.upstream_model, m.api_base, m.api_version,
                m.api_key_encrypted, m.catalog_key, m.pricing, m.is_active, m.created_at,
                m.weight, m.pricing_mode,
                (SELECT COUNT(*) FROM models d WHERE d.name = m.name) AS deployments
         FROM models m {WHERE} ORDER BY {order} LIMIT $4 OFFSET $5"
    )))
    .bind(provider)
    .bind(active)
    .bind(&search)
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    Ok(list.paged(rows, total))
}

pub async fn require(pool: &DbPool, id: i64) -> AppResult<LlmModel> {
    sqlx::query_as::<_, LlmModel>(concat!("SELECT ", columns!(), " FROM models WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("model {id}")))
}

/// The rates a model is billed by, and where they come from.
pub fn effective_pricing(model: &LlmModel, catalog: &Catalog) -> (Option<Pricing>, PricingSource) {
    match model.pricing_mode.as_str() {
        "free" => return (None, PricingSource::Free),
        "custom" => {
            if let Some(pricing) = model.custom_pricing() {
                return (Some(pricing), PricingSource::Custom);
            }
        }
        _ => {}
    }
    match model
        .catalog_key
        .as_deref()
        .and_then(|key| catalog.get(key))
    {
        Some(entry) => (Some(entry.pricing.clone()), PricingSource::Catalog),
        None => (None, PricingSource::None),
    }
}

pub fn to_response(model: LlmModel, catalog: &Catalog) -> ModelResponse {
    let (pricing, pricing_source) = effective_pricing(&model, catalog);
    ModelResponse {
        effective_api_base: model.effective_api_base(),
        has_api_key: model.api_key_encrypted.is_some(),
        custom_pricing: model.custom_pricing().as_ref().map(PricingUsd::from),
        pricing: pricing.as_ref().map(PricingUsd::from),
        pricing_source,
        pricing_mode: model.pricing_mode.clone(),
        id: model.id,
        name: model.name,
        provider: model.provider,
        upstream_model: model.upstream_model,
        api_base: model.api_base,
        api_version: model.api_version,
        catalog_key: model.catalog_key,
        is_active: model.is_active,
        created_at: model.created_at,
        weight: model.weight,
    }
}

/// An explicit catalog key must exist; without one, the provider's model id
/// is looked up the way the price list files it. `None` when nothing matches.
fn resolve_catalog_key(
    catalog: &Catalog,
    explicit: Option<&str>,
    provider: &str,
    upstream_model: &str,
) -> AppResult<Option<String>> {
    match explicit.map(str::trim).filter(|k| !k.is_empty()) {
        Some(key) if catalog.get(key).is_some() => Ok(Some(key.to_string())),
        Some(key) => Err(
            AppError::Validation(format!("'{key}' is not in the price catalog"))
                .with_field("catalog_key", FieldErrorCode::Invalid),
        ),
        None => Ok(catalog
            .find(provider, upstream_model)
            .map(|e| e.key.clone())),
    }
}

/// The pricing mode a request asks for, checked against what the model will
/// have: own rates need rates.
fn pricing_mode(mode: &str, has_rates: bool) -> AppResult<String> {
    if !PRICING_MODES.contains(&mode) {
        return Err(AppError::Validation(format!(
            "pricing_mode must be one of {}",
            PRICING_MODES.join(", ")
        ))
        .with_field("pricing_mode", FieldErrorCode::Invalid));
    }
    if mode == "custom" && !has_rates {
        return Err(
            AppError::Validation("own rates need rates: send `pricing`".into())
                .with_field("pricing", FieldErrorCode::Required),
        );
    }
    Ok(mode.to_string())
}

fn pricing_json(pricing: Option<&PricingUsd>) -> AppResult<Option<String>> {
    pricing
        .map(|p| {
            let rates = p.to_pricing()?;
            serde_json::to_string(&rates).map_err(|e| AppError::Internal(e.to_string()))
        })
        .transpose()
}

pub async fn create(
    pool: &DbPool,
    secrets: &SecretBox,
    catalog: &Catalog,
    req: &CreateModelRequest,
) -> AppResult<LlmModel> {
    let upstream_model = required_name(&req.upstream_model, "upstream_model", 200)?;
    let fields = Fields {
        name: validate_name(&req.name)?,
        provider: req.provider.clone(),
        api_base: validate_api_base(&req.provider, req.api_base.as_deref())?,
        api_version: validate_api_version(&req.provider, req.api_version.as_deref())?,
        api_key_encrypted: seal(secrets, req.api_key.as_deref())?,
        catalog_key: resolve_catalog_key(
            catalog,
            req.catalog_key.as_deref(),
            &req.provider,
            &upstream_model,
        )?,
        pricing: pricing_json(req.pricing.as_ref())?,
        upstream_model,
        is_active: true,
        weight: validate_weight(req.weight)?,
        pricing_mode: String::new(),
    };
    let fields = Fields {
        pricing_mode: pricing_mode(
            req.pricing_mode
                .as_deref()
                .unwrap_or(if req.pricing.is_some() {
                    "custom"
                } else {
                    "catalog"
                }),
            fields.pricing.is_some(),
        )?,
        ..fields
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO models (name, provider, upstream_model, api_base, api_key_encrypted,
                             catalog_key, pricing, is_active, created_at, api_version, weight,
                             pricing_mode)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id",
    )
    .bind(&fields.name)
    .bind(&fields.provider)
    .bind(&fields.upstream_model)
    .bind(&fields.api_base)
    .bind(&fields.api_key_encrypted)
    .bind(&fields.catalog_key)
    .bind(&fields.pricing)
    .bind(fields.is_active)
    .bind(Utc::now())
    .bind(&fields.api_version)
    .bind(fields.weight)
    .bind(&fields.pricing_mode)
    .fetch_one(pool)
    .await?;
    require(pool, id).await
}

pub async fn update(
    pool: &DbPool,
    secrets: &SecretBox,
    catalog: &Catalog,
    id: i64,
    req: &UpdateModelRequest,
) -> AppResult<LlmModel> {
    let current = require(pool, id).await?;
    let provider = req.provider.clone().unwrap_or(current.provider);
    let upstream_model = required_name(
        req.upstream_model
            .as_deref()
            .unwrap_or(&current.upstream_model),
        "upstream_model",
        200,
    )?;
    let api_base = match &req.api_base {
        Some(base) => base.clone(),
        None => current.api_base,
    };
    let catalog_key = match &req.catalog_key {
        Some(key) => resolve_catalog_key(catalog, key.as_deref(), &provider, &upstream_model)?,
        // A new provider model id re-resolves an automatic match.
        None if req.upstream_model.is_some() || req.provider.is_some() => {
            resolve_catalog_key(catalog, None, &provider, &upstream_model)?
        }
        None => current.catalog_key,
    };
    let fields = Fields {
        name: validate_name(req.name.as_deref().unwrap_or(&current.name))?,
        api_base: validate_api_base(&provider, api_base.as_deref())?,
        api_version: validate_api_version(
            &provider,
            match &req.api_version {
                Some(version) => version.as_deref(),
                None => current.api_version.as_deref(),
            },
        )?,
        provider,
        upstream_model,
        api_key_encrypted: match &req.api_key {
            Some(key) => seal(secrets, key.as_deref())?,
            None => current.api_key_encrypted,
        },
        catalog_key,
        pricing: match &req.pricing {
            Some(pricing) => pricing_json(pricing.as_ref())?,
            None => current.pricing,
        },
        is_active: req.is_active.unwrap_or(current.is_active),
        weight: match req.weight {
            Some(weight) => validate_weight(weight)?,
            None => current.weight,
        },
        pricing_mode: String::new(),
    };
    let mode = match (&req.pricing_mode, &req.pricing) {
        (Some(mode), _) => mode.as_str(),
        (None, Some(Some(_))) => "custom",
        (None, Some(None)) => "catalog",
        (None, None) => current.pricing_mode.as_str(),
    };
    let fields = Fields {
        pricing_mode: pricing_mode(mode, fields.pricing.is_some())?,
        ..fields
    };
    sqlx::query(
        "UPDATE models SET name = $1, provider = $2, upstream_model = $3, api_base = $4,
                api_key_encrypted = $5, catalog_key = $6, pricing = $7,
                is_active = $8, api_version = $10, weight = $11, pricing_mode = $12
         WHERE id = $9",
    )
    .bind(&fields.name)
    .bind(&fields.provider)
    .bind(&fields.upstream_model)
    .bind(&fields.api_base)
    .bind(&fields.api_key_encrypted)
    .bind(&fields.catalog_key)
    .bind(&fields.pricing)
    .bind(fields.is_active)
    .bind(id)
    .bind(&fields.api_version)
    .bind(fields.weight)
    .bind(&fields.pricing_mode)
    .execute(pool)
    .await?;
    require(pool, id).await
}

pub async fn delete(pool: &DbPool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query("DELETE FROM models WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(AppError::NotFound(format!("model {id}")));
    }
    Ok(())
}

/// `weight`: zero or more; zero is picked only when nothing else
/// of its name is healthy.
fn validate_weight(weight: Option<i64>) -> AppResult<Option<i64>> {
    if weight.is_some_and(|w| !(0..=1_000_000).contains(&w)) {
        return Err(
            AppError::Validation("weight must be between 0 and 1000000".into())
                .with_field("weight", FieldErrorCode::Invalid),
        );
    }
    Ok(weight)
}

fn seal(secrets: &SecretBox, key: Option<&str>) -> AppResult<Option<String>> {
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(key) => secrets.encrypt(key).map(Some),
        None => Ok(None),
    }
}

/// The name clients send as `model`: letters, digits and `. _ - : /`, so it
/// survives URLs, shells and config files untouched.
fn validate_name(name: &str) -> AppResult<String> {
    let name = required_name(name, "name", 100)?;
    let allowed = |c: char| c.is_ascii_alphanumeric() || "._-:/".contains(c);
    if !name.chars().all(allowed) {
        return Err(AppError::Validation(
            "name may only contain letters, digits and . _ - : /".into(),
        )
        .with_field("name", FieldErrorCode::Invalid));
    }
    Ok(name)
}

/// Azure routes by API version; nobody else takes one.
fn validate_api_version(provider: &str, version: Option<&str>) -> AppResult<Option<String>> {
    let version = version.map(str::trim).filter(|v| !v.is_empty());
    match (provider, version) {
        ("azure", None) => Err(AppError::Validation(
            "Azure models need an api_version, such as 2024-10-21".into(),
        )
        .with_field("api_version", FieldErrorCode::Required)),
        ("azure", Some(version))
            if version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') =>
        {
            Ok(Some(version.to_string()))
        }
        ("azure", Some(version)) => Err(AppError::Validation(format!(
            "'{version}' is not an API version"
        ))
        .with_field("api_version", FieldErrorCode::Invalid)),
        _ => Ok(None),
    }
}

/// Checks the provider exists and the base URL is http(s). Returns the base
/// without a trailing slash, or `None` when the provider's default applies.
fn validate_api_base(provider: &str, api_base: Option<&str>) -> AppResult<Option<String>> {
    let Some(known) = providers::find(provider) else {
        return Err(
            AppError::Validation(format!("unknown provider '{provider}'"))
                .with_field("provider", FieldErrorCode::Invalid),
        );
    };
    let api_base = api_base.map(str::trim).filter(|b| !b.is_empty());
    match api_base {
        None if known.default_api_base.is_none() => Err(AppError::Validation(format!(
            "provider '{provider}' needs an api_base"
        ))
        .with_field("api_base", FieldErrorCode::Required)),
        None => Ok(None),
        Some(base) => {
            let parsed = reqwest::Url::parse(base).ok();
            let valid = parsed
                .as_ref()
                .is_some_and(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some());
            if !valid {
                return Err(
                    AppError::Validation(format!("'{base}' is not an http(s) URL"))
                        .with_field("api_base", FieldErrorCode::Invalid),
                );
            }
            Ok(Some(base.trim_end_matches('/').to_string()))
        }
    }
}

/// A throwaway route for "Test Connect", validated like a saved model. A
/// blank key on an existing model means its stored one.
pub async fn probe_route(
    pool: &DbPool,
    secrets: &SecretBox,
    catalog: &Catalog,
    req: &TestModelRequest,
) -> AppResult<(crate::gateway::Route, bool)> {
    let provider = providers::find(&req.provider).ok_or_else(|| {
        AppError::Validation(format!("unknown provider '{}'", req.provider))
            .with_field("provider", FieldErrorCode::Invalid)
    })?;
    let upstream_model = required_name(&req.upstream_model, "upstream_model", 200)?;
    let api_base = validate_api_base(&req.provider, req.api_base.as_deref())?
        .or_else(|| provider.default_api_base.map(String::from))
        .unwrap_or_default();
    let api_version = validate_api_version(&req.provider, req.api_version.as_deref())?;
    let typed = req
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty());
    let api_key = match (typed, req.model_id) {
        (Some(key), _) => Some(key.to_string()),
        (None, Some(id)) => match require(pool, id).await?.api_key_encrypted {
            Some(sealed) => Some(secrets.decrypt(&sealed)?),
            None => None,
        },
        (None, None) => None,
    };
    let embedding = catalog
        .find(&req.provider, &upstream_model)
        .is_some_and(|e| e.mode == "embedding");
    let route = crate::gateway::Route {
        id: req.model_id.unwrap_or_default(),
        weight: None,
        name: upstream_model.clone(),
        provider: req.provider.clone(),
        wire: provider.wire,
        stream_usage: provider.stream_usage,
        native_responses: provider.native_responses,
        upstream_model,
        api_base,
        api_version,
        api_key,
        pricing: Pricing::default(),
        max_output_tokens: None,
        supports_native_structured_output: false,
        created_at: Utc::now(),
    };
    Ok((route, embedding))
}
