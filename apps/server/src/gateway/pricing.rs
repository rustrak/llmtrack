//! What a request costs.
//!
//! Rates are integers: micro-USD per million tokens (the unit providers quote
//! in, `$2.50 / 1M` is `2_500_000`), micro-USD per image or second, and
//! micro-USD per million characters. A token at a rate of R micro-USD per
//! million costs R pico-USD, so a whole request sums in pico-USD and becomes
//! nano-USD (the unit spend is stored in) with one division at the end.
//!
//! The rules:
//! - `prompt_tokens` includes cache reads and cache writes; those are taken
//!   out and billed at their own rates, the rest is billed as input.
//! - `completion_tokens` includes reasoning; reasoning is billed at its own
//!   rate when there is one, at the output rate otherwise.
//! - A missing cache or reasoning rate falls back to the input or output rate;
//!   an explicit zero is a real price.
//! - Past `above.threshold_tokens` of prompt, every rate switches to the
//!   long-context tier at once (each falling back to its base rate).

use serde::{Deserialize, Serialize};

/// Token counts as the provider reported them, normalised to OpenAI's
/// convention (prompt includes cache, completion includes reasoning).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    /// Prompt tokens served from the provider's cache.
    pub cached_tokens: i64,
    /// Prompt tokens written to the cache (all durations).
    pub cache_write_tokens: i64,
    /// The part of `cache_write_tokens` cached for an hour (Anthropic).
    pub cache_write_1h_tokens: i64,
    /// The part of `completion_tokens` spent reasoning.
    pub reasoning_tokens: i64,
    /// Images generated, for per-image pricing.
    pub images: i64,
    /// Audio seconds, for per-second pricing.
    pub seconds: i64,
    /// Characters of input, for per-character pricing (text to speech).
    pub characters: i64,
}

/// Rates for one context tier, micro-USD per million tokens.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenRates {
    pub input: i64,
    pub output: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_1h: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<i64>,
}

/// The long-context tier: prompts larger than `threshold_tokens`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboveTier {
    pub threshold_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<i64>,
}

/// Everything a model can be billed by.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pricing {
    #[serde(flatten)]
    pub tokens: TokenRates,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<AboveTier>,
    /// Micro-USD per generated image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_image: Option<i64>,
    /// Micro-USD per second of audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_second: Option<i64>,
    /// Micro-USD per million input characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_million_characters: Option<i64>,
}

impl Pricing {
    pub fn tokens(input: i64, output: i64) -> Self {
        Self {
            tokens: TokenRates {
                input,
                output,
                ..TokenRates::default()
            },
            ..Self::default()
        }
    }

    /// The token rates that apply to a prompt of this size.
    fn rates_for(&self, prompt_tokens: i64) -> TokenRates {
        let base = &self.tokens;
        match &self.above {
            Some(above) if prompt_tokens > above.threshold_tokens => TokenRates {
                input: above.input.unwrap_or(base.input),
                output: above.output.unwrap_or(base.output),
                cache_read: above.cache_read.or(base.cache_read),
                cache_write: above.cache_write.or(base.cache_write),
                cache_write_1h: base.cache_write_1h,
                reasoning: base.reasoning,
            },
            _ => base.clone(),
        }
    }

    /// The cost of `usage`, in nano-USD.
    pub fn cost_nanos(&self, usage: &Usage) -> i64 {
        let rates = self.rates_for(usage.prompt_tokens);
        let cache_read = rates.cache_read.unwrap_or(rates.input);
        let cache_write = rates.cache_write.unwrap_or(rates.input);
        let cache_write_1h = rates.cache_write_1h.unwrap_or(cache_write);
        let reasoning_rate = rates.reasoning.unwrap_or(rates.output);

        let cached = usage.cached_tokens.max(0);
        let written = usage.cache_write_tokens.max(0);
        let written_1h = usage.cache_write_1h_tokens.clamp(0, written);
        let text_in = (usage.prompt_tokens - cached - written).max(0);
        let reasoning = usage
            .reasoning_tokens
            .clamp(0, usage.completion_tokens.max(0));
        let text_out = (usage.completion_tokens - reasoning).max(0);

        let mul = |a: i64, b: i64| i128::from(a) * i128::from(b);
        let picos = mul(text_in, rates.input)
            + mul(cached, cache_read)
            + mul(written - written_1h, cache_write)
            + mul(written_1h, cache_write_1h)
            + mul(text_out, rates.output)
            + mul(reasoning, reasoning_rate)
            + mul(usage.characters, self.per_million_characters.unwrap_or(0))
            // Per-unit rates are micro-USD per unit: a million picos each.
            + mul(usage.images, self.per_image.unwrap_or(0)) * 1_000_000
            + mul(usage.seconds, self.per_second.unwrap_or(0)) * 1_000_000;
        i64::try_from(picos / 1000).unwrap_or(i64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(prompt: i64, completion: i64) -> Usage {
        Usage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            ..Usage::default()
        }
    }

    /// claude-sonnet-4-5 as the price list prices it.
    fn sonnet() -> Pricing {
        Pricing {
            tokens: TokenRates {
                input: 3_000_000,
                output: 15_000_000,
                cache_read: Some(300_000),
                cache_write: Some(3_750_000),
                cache_write_1h: Some(6_000_000),
                reasoning: None,
            },
            above: Some(AboveTier {
                threshold_tokens: 200_000,
                input: Some(6_000_000),
                output: Some(22_500_000),
                cache_read: Some(600_000),
                cache_write: Some(7_500_000),
            }),
            ..Pricing::default()
        }
    }

    #[test]
    fn plain_input_and_output() {
        // 1000 × $2.50/M + 1000 × $10/M = $0.0125
        assert_eq!(
            Pricing::tokens(2_500_000, 10_000_000).cost_nanos(&usage(1000, 1000)),
            12_500_000
        );
    }

    #[test]
    fn cache_reads_and_writes_are_taken_out_of_the_prompt() {
        let u = Usage {
            prompt_tokens: 10_000,
            cached_tokens: 8_000,
            cache_write_tokens: 1_000,
            ..usage(10_000, 0)
        };
        // 1000 text × $3 + 8000 read × $0.30 + 1000 write × $3.75 (per M)
        let expected = 1_000 * 3_000 + 8_000 * 300 + 1_000 * 3_750;
        assert_eq!(sonnet().cost_nanos(&u), expected);
    }

    #[test]
    fn hour_long_cache_writes_have_their_own_rate() {
        let u = Usage {
            cache_write_tokens: 1_000,
            cache_write_1h_tokens: 400,
            ..usage(1_000, 0)
        };
        assert_eq!(sonnet().cost_nanos(&u), 600 * 3_750 + 400 * 6_000);
    }

    #[test]
    fn missing_cache_rates_fall_back_to_input_but_zero_is_a_price() {
        let mut p = Pricing::tokens(1_000_000, 0);
        let u = Usage {
            cached_tokens: 100,
            ..usage(100, 0)
        };
        assert_eq!(p.cost_nanos(&u), 100 * 1_000);
        p.tokens.cache_read = Some(0);
        assert_eq!(p.cost_nanos(&u), 0);
    }

    #[test]
    fn reasoning_bills_at_its_own_rate_or_the_output_rate() {
        let mut p = Pricing::tokens(0, 10_000_000);
        let u = Usage {
            reasoning_tokens: 300,
            ..usage(0, 1_000)
        };
        assert_eq!(p.cost_nanos(&u), 1_000 * 10_000);
        p.tokens.reasoning = Some(20_000_000);
        assert_eq!(p.cost_nanos(&u), 700 * 10_000 + 300 * 20_000);
    }

    #[test]
    fn a_long_prompt_switches_every_rate_to_the_upper_tier() {
        let short = sonnet().cost_nanos(&usage(200_000, 1_000));
        assert_eq!(short, 200_000 * 3_000 + 1_000 * 15_000);
        let long = sonnet().cost_nanos(&usage(200_001, 1_000));
        assert_eq!(long, 200_001 * 6_000 + 1_000 * 22_500);
    }

    #[test]
    fn per_unit_prices() {
        let p = Pricing {
            per_image: Some(40_000),                  // $0.04 per image
            per_second: Some(100),                    // $0.0001 per second
            per_million_characters: Some(15_000_000), // $15 per 1M chars
            ..Pricing::default()
        };
        let u = Usage {
            images: 2,
            seconds: 60,
            characters: 1_000,
            ..Usage::default()
        };
        assert_eq!(p.cost_nanos(&u), 80_000_000 + 6_000_000 + 15_000_000);
    }

    #[test]
    fn nonsense_counts_never_go_negative() {
        let u = Usage {
            cached_tokens: 500,
            reasoning_tokens: 99,
            ..usage(100, 10)
        };
        assert_eq!(
            Pricing::tokens(1_000_000, 1_000_000).cost_nanos(&u),
            500 * 1_000 + 10 * 1_000
        );
    }

    #[test]
    fn serializes_without_empty_rates() {
        let json = serde_json::to_value(Pricing::tokens(1, 2)).unwrap();
        assert_eq!(json, serde_json::json!({"input": 1, "output": 2}));
    }
}
