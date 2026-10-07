use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::money::{micros_to_usd, usd_to_micros};
use super::patch;
use crate::error::AppResult;
use crate::gateway::pricing::{AboveTier, Pricing, TokenRates};
use crate::gateway::providers;

/// A model clients can ask for by `name`, and where the gateway sends it.
#[derive(Debug, Clone, FromRow)]
pub struct LlmModel {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub upstream_model: String,
    pub api_base: Option<String>,
    pub api_version: Option<String>,
    pub api_key_encrypted: Option<String>,
    /// The price-list entry it is priced by, when it has no own rates.
    pub catalog_key: Option<String>,
    /// Its own rates (JSON of [`Pricing`]), which win over the catalog.
    pub pricing: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    /// Share of its name's traffic, relative to the other deployments of
    /// the name (`weight`); `None` shares evenly.
    pub weight: Option<i64>,
    /// `catalog`, `custom` or `free`; see [`PricingSource`].
    pub pricing_mode: String,
}

impl LlmModel {
    /// The configured base, or the provider's default.
    pub fn effective_api_base(&self) -> Option<String> {
        self.api_base.clone().or_else(|| {
            providers::find(&self.provider)
                .and_then(|p| p.default_api_base)
                .map(String::from)
        })
    }

    pub fn custom_pricing(&self) -> Option<Pricing> {
        self.pricing
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
    }
}

/// How a model is meant to be priced, chosen by an admin.
pub const PRICING_MODES: [&str; 3] = ["catalog", "custom", "free"];

/// Where a model's prices come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    /// Rates set on the model.
    Custom,
    /// The price-list entry `catalog_key`.
    Catalog,
    /// Deliberately unpriced: requests are logged at $0.
    Free,
    /// Meant to be priced by the catalog, which does not list it: logged at
    /// $0 too, and the dashboard warns about it.
    None,
}

/// Rates as the API reads and writes them: USD per million tokens, USD per
/// image or second, USD per million characters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PricingUsd {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_1h: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<AboveTierUsd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_image: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_million_characters: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AboveTierUsd {
    pub threshold_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

fn opt_micros(usd: Option<f64>, field: &str) -> AppResult<Option<i64>> {
    usd.map(|v| usd_to_micros(v, field)).transpose()
}

fn opt_usd(micros: Option<i64>) -> Option<f64> {
    micros.map(micros_to_usd)
}

impl PricingUsd {
    /// Validates and converts to integer rates. Field names in errors are
    /// the JSON paths the dashboard pins them to.
    pub fn to_pricing(&self) -> AppResult<Pricing> {
        let above = match &self.above {
            Some(a) => {
                if a.threshold_tokens <= 0 {
                    return Err(crate::error::AppError::Validation(
                        "the long-context threshold must be a positive token count".into(),
                    )
                    .with_field(
                        "pricing.above.threshold_tokens",
                        crate::error::FieldErrorCode::Invalid,
                    ));
                }
                Some(AboveTier {
                    threshold_tokens: a.threshold_tokens,
                    input: opt_micros(a.input, "pricing.above.input")?,
                    output: opt_micros(a.output, "pricing.above.output")?,
                    cache_read: opt_micros(a.cache_read, "pricing.above.cache_read")?,
                    cache_write: opt_micros(a.cache_write, "pricing.above.cache_write")?,
                })
            }
            None => None,
        };
        Ok(Pricing {
            tokens: TokenRates {
                input: usd_to_micros(self.input, "pricing.input")?,
                output: usd_to_micros(self.output, "pricing.output")?,
                cache_read: opt_micros(self.cache_read, "pricing.cache_read")?,
                cache_write: opt_micros(self.cache_write, "pricing.cache_write")?,
                cache_write_1h: opt_micros(self.cache_write_1h, "pricing.cache_write_1h")?,
                reasoning: opt_micros(self.reasoning, "pricing.reasoning")?,
            },
            above,
            per_image: opt_micros(self.per_image, "pricing.per_image")?,
            per_second: opt_micros(self.per_second, "pricing.per_second")?,
            per_million_characters: opt_micros(
                self.per_million_characters,
                "pricing.per_million_characters",
            )?,
        })
    }
}

impl From<&Pricing> for PricingUsd {
    fn from(p: &Pricing) -> Self {
        Self {
            input: micros_to_usd(p.tokens.input),
            output: micros_to_usd(p.tokens.output),
            cache_read: opt_usd(p.tokens.cache_read),
            cache_write: opt_usd(p.tokens.cache_write),
            cache_write_1h: opt_usd(p.tokens.cache_write_1h),
            reasoning: opt_usd(p.tokens.reasoning),
            above: p.above.as_ref().map(|a| AboveTierUsd {
                threshold_tokens: a.threshold_tokens,
                input: opt_usd(a.input),
                output: opt_usd(a.output),
                cache_read: opt_usd(a.cache_read),
                cache_write: opt_usd(a.cache_write),
            }),
            per_image: opt_usd(p.per_image),
            per_second: opt_usd(p.per_second),
            per_million_characters: opt_usd(p.per_million_characters),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ModelResponse {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub upstream_model: String,
    pub api_base: Option<String>,
    pub api_version: Option<String>,
    pub effective_api_base: Option<String>,
    pub has_api_key: bool,
    pub catalog_key: Option<String>,
    /// The rates set on the model, if any.
    pub custom_pricing: Option<PricingUsd>,
    /// The rates requests are billed by, wherever they come from.
    pub pricing: Option<PricingUsd>,
    pub pricing_source: PricingSource,
    pub pricing_mode: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub weight: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateModelRequest {
    pub name: String,
    pub provider: String,
    pub upstream_model: String,
    pub api_base: Option<String>,
    pub api_version: Option<String>,
    pub api_key: Option<String>,
    /// A price-list key; looked up from the provider and model when absent.
    pub catalog_key: Option<String>,
    /// Own rates; when present they win over the catalog.
    pub pricing: Option<PricingUsd>,
    pub weight: Option<i64>,
    /// `catalog`, `custom` or `free`. Defaults to `custom` when rates are
    /// given, `catalog` otherwise.
    pub pricing_mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub upstream_model: Option<String>,
    #[serde(default, deserialize_with = "patch")]
    pub api_base: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch")]
    pub api_version: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch")]
    pub api_key: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch")]
    pub catalog_key: Option<Option<String>>,
    /// `null` drops the own rates and goes back to the catalog.
    #[serde(default, deserialize_with = "patch")]
    pub pricing: Option<Option<PricingUsd>>,
    pub is_active: Option<bool>,
    #[serde(default, deserialize_with = "patch")]
    pub weight: Option<Option<i64>>,
    /// Without it, new rates switch to `custom` and `pricing: null` to
    /// `catalog`.
    pub pricing_mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usd_rates_round_trip_through_micros() {
        let usd = PricingUsd {
            input: 3.0,
            output: 15.0,
            cache_read: Some(0.3),
            above: Some(AboveTierUsd {
                threshold_tokens: 200_000,
                input: Some(6.0),
                ..AboveTierUsd::default()
            }),
            per_image: Some(0.04),
            ..PricingUsd::default()
        };
        let pricing = usd.to_pricing().unwrap();
        assert_eq!(pricing.tokens.cache_read, Some(300_000));
        assert_eq!(pricing.per_image, Some(40_000));
        assert_eq!(PricingUsd::from(&pricing), usd);
    }

    #[test]
    fn negative_rates_name_their_path() {
        let err = PricingUsd {
            cache_read: Some(-1.0),
            ..PricingUsd::default()
        }
        .to_pricing()
        .unwrap_err();
        let body = format!("{err:?}");
        assert!(body.contains("pricing.cache_read"), "{body}");
    }
}

/// "Test Connect" from the model form: the settings being edited, and the
/// model they belong to when it already exists (for its stored key).
#[derive(Debug, Deserialize)]
pub struct TestModelRequest {
    pub model_id: Option<i64>,
    pub provider: String,
    pub upstream_model: String,
    pub api_base: Option<String>,
    pub api_version: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModelListQuery {
    pub provider: Option<String>,
    /// `active` or `inactive`.
    pub status: Option<String>,
}

/// A row of the models table: the model and how many share its name.
#[derive(Debug, Serialize)]
pub struct ModelListItem {
    #[serde(flatten)]
    pub model: ModelResponse,
    pub deployments: i64,
}
