//! The price list, read as-is.
//!
//! The market's prices live in one JSON file, `model_prices.json` (USD per
//! token, plus capability flags). A snapshot ships inside the binary so
//! pricing works offline; an admin can refresh it from the list's URL, and
//! the refreshed copy is kept in the database (see `services::catalog`).
//!
//! Only what billing and translation need is kept from each entry.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

use super::pricing::{AboveTier, Pricing, TokenRates};

/// The snapshot shipped with this build (gzip of the JSON).
const SNAPSHOT: &[u8] = include_bytes!("../../assets/model_prices.json.gz");

pub const DEFAULT_URL: &str =
    "https://raw.githubusercontent.com/rustrak/llmtrack/main/pricing/model_prices.json";

/// A catalog this small is not the price list: refuse it rather than wipe prices.
pub const MIN_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub key: String,
    pub provider: String,
    pub mode: String,
    pub pricing: Pricing,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub supports_reasoning: bool,
    pub supports_prompt_caching: bool,
    pub supports_vision: bool,
    pub supports_function_calling: bool,
    pub supports_native_structured_output: bool,
}

#[derive(Debug, Default)]
pub struct Catalog {
    entries: HashMap<String, CatalogEntry>,
}

impl Catalog {
    /// The snapshot compiled into this binary, parsed once per process.
    pub fn builtin() -> std::sync::Arc<Self> {
        static BUILTIN: std::sync::OnceLock<std::sync::Arc<Catalog>> = std::sync::OnceLock::new();
        BUILTIN
            .get_or_init(|| {
                use std::io::Read;
                let mut json = String::new();
                flate2::read::GzDecoder::new(SNAPSHOT)
                    .read_to_string(&mut json)
                    .expect("the bundled price snapshot is valid gzip");
                std::sync::Arc::new(
                    Self::parse(&json).expect("the bundled price snapshot is valid"),
                )
            })
            .clone()
    }

    /// Reads the JSON. Entries without any price are skipped.
    pub fn parse(json: &str) -> Result<Self, String> {
        let root: Map<String, Value> =
            serde_json::from_str(json).map_err(|e| format!("not a price list: {e}"))?;
        let entries: HashMap<String, CatalogEntry> = root
            .iter()
            .filter(|(key, _)| key.as_str() != "sample_spec")
            .filter_map(|(key, value)| Some((key.clone(), entry(key, value.as_object()?)?)))
            .collect();
        if entries.len() < MIN_ENTRIES {
            return Err(format!(
                "only {} priced models; expected the full price list",
                entries.len()
            ));
        }
        Ok(Self { entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&CatalogEntry> {
        self.entries.get(key)
    }

    /// The entry for a provider's model id, trying the keys the list files it
    /// under: `{prefix}/{model}` first, then the bare id.
    pub fn find(&self, provider: &str, upstream_model: &str) -> Option<&CatalogEntry> {
        let prefixed = catalog_prefix(provider).map(|prefix| format!("{prefix}/{upstream_model}"));
        prefixed
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(upstream_model))
            .find_map(|key| self.entries.get(key))
    }

    /// The models the list has for one of our providers, filtered by the
    /// words of `query`, each with the id the provider itself expects.
    pub fn models_for(
        &self,
        provider: &str,
        query: &str,
        limit: usize,
    ) -> Vec<(&CatalogEntry, String)> {
        let Some(listed_as) = catalog_provider(provider) else {
            return Vec::new();
        };
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let prefix = catalog_prefix(provider).map(|p| format!("{p}/"));
        let mut hits: Vec<(&CatalogEntry, String)> = self
            .entries
            .values()
            .filter(|e| e.provider == listed_as)
            .filter(|e| words.iter().all(|w| e.key.to_lowercase().contains(w)))
            .map(|e| {
                let model = prefix
                    .as_deref()
                    .and_then(|p| e.key.strip_prefix(p))
                    .unwrap_or(&e.key)
                    .to_string();
                (e, model)
            })
            .collect();
        hits.sort_by(|a, b| a.1.cmp(&b.1));
        hits.truncate(limit);
        hits
    }

    /// Entries whose key contains every word of `query`, shortest key first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&CatalogEntry> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits: Vec<&CatalogEntry> = self
            .entries
            .values()
            .filter(|e| {
                let key = e.key.to_lowercase();
                words
                    .iter()
                    .all(|w| key.contains(w) || e.provider.contains(w.as_str()))
            })
            .collect();
        hits.sort_by(|a, b| {
            a.key
                .len()
                .cmp(&b.key.len())
                .then_with(|| a.key.cmp(&b.key))
        });
        hits.truncate(limit);
        hits
    }
}

/// The `litellm_provider` the list files our providers' models under.
fn catalog_provider(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "openai" => "openai",
        "anthropic" => "anthropic",
        "azure" => "azure",
        "gemini" => "gemini",
        "groq" => "groq",
        "mistral" => "mistral",
        "deepseek" => "deepseek",
        "openrouter" => "openrouter",
        "together" => "together_ai",
        "ollama" => "ollama",
        _ => return None,
    })
}

/// How the list prefixes keys for our providers. OpenAI, Anthropic and Azure
/// deployments of OpenAI models are also listed bare.
fn catalog_prefix(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "azure" => "azure",
        "gemini" => "gemini",
        "groq" => "groq",
        "mistral" => "mistral",
        "deepseek" => "deepseek",
        "openrouter" => "openrouter",
        "together" => "together_ai",
        "ollama" => "ollama",
        _ => return None,
    })
}

/// USD per token → micro-USD per million tokens.
fn per_mtok(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|usd| (usd * 1e12).round() as i64)
}

/// USD per unit → micro-USD per unit.
fn per_unit(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|usd| (usd * 1e6).round() as i64)
}

/// The lowest `_above_{N}k_tokens` threshold an entry prices.
// ponytail: one long-context tier; entries with several (128k and 256k) keep the first.
fn above_tier(e: &Map<String, Value>) -> Option<AboveTier> {
    let threshold = e
        .keys()
        .filter_map(|k| k.strip_prefix("input_cost_per_token_above_"))
        .filter_map(|rest| rest.strip_suffix("k_tokens"))
        .filter_map(|n| n.parse::<i64>().ok())
        .min()?;
    let rate = |base: &str| per_mtok(e.get(&format!("{base}_above_{threshold}k_tokens")));
    Some(AboveTier {
        threshold_tokens: threshold * 1000,
        input: rate("input_cost_per_token"),
        output: rate("output_cost_per_token"),
        cache_read: rate("cache_read_input_token_cost"),
        cache_write: rate("cache_creation_input_token_cost"),
    })
}

fn entry(key: &str, e: &Map<String, Value>) -> Option<CatalogEntry> {
    let input = per_mtok(e.get("input_cost_per_token"));
    let output = per_mtok(e.get("output_cost_per_token"));
    let per_image = per_unit(e.get("output_cost_per_image"));
    let per_second = per_unit(e.get("input_cost_per_second"))
        .or_else(|| per_unit(e.get("output_cost_per_second")));
    let per_million_characters = per_mtok(e.get("input_cost_per_character"));
    if input.is_none()
        && output.is_none()
        && per_image.is_none()
        && per_second.is_none()
        && per_million_characters.is_none()
    {
        return None;
    }
    let flag = |name: &str| e.get(name).and_then(Value::as_bool).unwrap_or(false);
    let int = |name: &str| e.get(name).and_then(Value::as_i64);
    Some(CatalogEntry {
        key: key.to_string(),
        provider: e
            .get("litellm_provider")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        mode: e
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("chat")
            .to_string(),
        pricing: Pricing {
            tokens: TokenRates {
                input: input.unwrap_or(0),
                output: output.unwrap_or(0),
                cache_read: per_mtok(e.get("cache_read_input_token_cost")),
                cache_write: per_mtok(e.get("cache_creation_input_token_cost")),
                cache_write_1h: per_mtok(e.get("cache_creation_input_token_cost_above_1hr")),
                reasoning: per_mtok(e.get("output_cost_per_reasoning_token")),
            },
            above: above_tier(e),
            per_image,
            per_second,
            per_million_characters,
        },
        max_input_tokens: int("max_input_tokens"),
        max_output_tokens: int("max_output_tokens").or_else(|| int("max_tokens")),
        supports_reasoning: flag("supports_reasoning"),
        supports_prompt_caching: flag("supports_prompt_caching"),
        supports_vision: flag("supports_vision"),
        supports_function_calling: flag("supports_function_calling"),
        supports_native_structured_output: flag("supports_native_structured_output"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_snapshot_prices_the_usual_models() {
        let catalog = Catalog::builtin();
        assert!(catalog.len() > 1000, "{}", catalog.len());

        let gpt = catalog.find("openai", "gpt-4o").expect("gpt-4o");
        assert_eq!(gpt.pricing.tokens.input, 2_500_000);
        assert_eq!(gpt.pricing.tokens.output, 10_000_000);
        assert_eq!(gpt.pricing.tokens.cache_read, Some(1_250_000));

        let sonnet = catalog
            .find("anthropic", "claude-sonnet-4-5")
            .expect("sonnet");
        assert_eq!(sonnet.pricing.tokens.cache_write, Some(3_750_000));
        assert_eq!(
            sonnet.pricing.above.as_ref().unwrap().threshold_tokens,
            200_000
        );
        assert!(sonnet.supports_reasoning);
        assert!(sonnet.max_output_tokens.unwrap() >= 64_000);
    }

    #[test]
    fn provider_prefixed_keys_are_tried_first() {
        let catalog = Catalog::builtin();
        let gemini = catalog.find("gemini", "gemini-2.5-pro").expect("gemini");
        assert_eq!(gemini.key, "gemini/gemini-2.5-pro");
        assert!(catalog.find("openai", "no-such-model").is_none());
    }

    #[test]
    fn search_matches_every_word() {
        let catalog = Catalog::builtin();
        let hits = catalog.search("sonnet 4-5", 10);
        assert!(!hits.is_empty());
        assert!(hits
            .iter()
            .all(|e| e.key.contains("sonnet") && e.key.contains("4-5")));
    }

    #[test]
    fn a_tiny_or_broken_list_is_refused() {
        assert!(Catalog::parse("{}").is_err());
        assert!(Catalog::parse("not json").is_err());
    }

    #[test]
    fn per_unit_prices_are_read() {
        let mut root = Map::new();
        for i in 0..MIN_ENTRIES {
            root.insert(
                format!("m{i}"),
                serde_json::json!({"input_cost_per_token": 1e-6}),
            );
        }
        root.insert(
            "dall-e-3".into(),
            serde_json::json!({"output_cost_per_image": 0.04, "mode": "image_generation"}),
        );
        root.insert(
            "tts-1".into(),
            serde_json::json!({"input_cost_per_character": 1.5e-5, "mode": "audio_speech"}),
        );
        let catalog = Catalog::parse(&Value::Object(root).to_string()).unwrap();
        assert_eq!(
            catalog.get("dall-e-3").unwrap().pricing.per_image,
            Some(40_000)
        );
        assert_eq!(
            catalog.get("tts-1").unwrap().pricing.per_million_characters,
            Some(15_000_000)
        );
    }
}
