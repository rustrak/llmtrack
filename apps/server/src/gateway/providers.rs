//! The providers a model can point at.
//!
//! Most of the market speaks OpenAI's chat completions dialect, so a provider
//! is usually just a default base URL. Only Anthropic needs translating.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Wire {
    /// `POST {base}/chat/completions` with `Authorization: Bearer`.
    OpenAi,
    /// `POST {base}/messages` with `x-api-key`, translated both ways.
    Anthropic,
    /// OpenAI's dialect at `{base}/openai/deployments/{deployment}/…
    /// ?api-version=…` with an `api-key` header.
    Azure,
}

#[derive(Debug, Serialize)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub wire: Wire,
    /// `None` means the model must say where its server is.
    pub default_api_base: Option<&'static str>,
    /// Accepts `stream_options.include_usage`; false for Mistral,
    /// Gemini and Ollama: their streams are metered from whatever
    /// usage they send, or estimated.
    pub stream_usage: bool,
    /// Serves `/responses` itself; everyone else gets it translated to chat
    /// completions.
    pub native_responses: bool,
}

pub const PROVIDERS: &[Provider] = &[
    p(
        "openai",
        "OpenAI",
        Wire::OpenAi,
        Some("https://api.openai.com/v1"),
    ),
    p(
        "anthropic",
        "Anthropic",
        Wire::Anthropic,
        Some("https://api.anthropic.com/v1"),
    ),
    p("azure", "Azure OpenAI", Wire::Azure, None),
    p(
        "gemini",
        "Google Gemini",
        Wire::OpenAi,
        Some("https://generativelanguage.googleapis.com/v1beta/openai"),
    ),
    p(
        "groq",
        "Groq",
        Wire::OpenAi,
        Some("https://api.groq.com/openai/v1"),
    ),
    p(
        "mistral",
        "Mistral",
        Wire::OpenAi,
        Some("https://api.mistral.ai/v1"),
    ),
    p(
        "deepseek",
        "DeepSeek",
        Wire::OpenAi,
        Some("https://api.deepseek.com/v1"),
    ),
    p(
        "openrouter",
        "OpenRouter",
        Wire::OpenAi,
        Some("https://openrouter.ai/api/v1"),
    ),
    p(
        "together",
        "Together AI",
        Wire::OpenAi,
        Some("https://api.together.xyz/v1"),
    ),
    p(
        "ollama",
        "Ollama",
        Wire::OpenAi,
        Some("http://localhost:11434/v1"),
    ),
    p("openai_compatible", "OpenAI-compatible", Wire::OpenAi, None),
];

const fn p(
    id: &'static str,
    label: &'static str,
    wire: Wire,
    default_api_base: Option<&'static str>,
) -> Provider {
    Provider {
        id,
        label,
        wire,
        default_api_base,
        stream_usage: !matches!(id.as_bytes(), b"mistral" | b"gemini" | b"ollama"),
        native_responses: matches!(id.as_bytes(), b"openai" | b"azure"),
    }
}

pub fn find(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|provider| provider.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PROVIDERS.len());
    }

    #[test]
    fn only_anthropic_needs_translation() {
        assert_eq!(find("anthropic").unwrap().wire, Wire::Anthropic);
        assert_eq!(find("groq").unwrap().wire, Wire::OpenAi);
        assert!(find("openai_compatible")
            .unwrap()
            .default_api_base
            .is_none());
        assert_eq!(find("azure").unwrap().wire, Wire::Azure);
        assert!(find("bedrock").is_none());
        assert!(find("openai").unwrap().stream_usage);
        assert!(!find("mistral").unwrap().stream_usage);
    }
}
