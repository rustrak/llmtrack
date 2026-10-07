//! Money is stored as integers and only becomes a float at the API edge.
//!
//! - Prices: micro-USD per million tokens (`$2.50 / 1M` is `2_500_000`).
//! - Spend and budgets: nano-USD. One token at the cheapest price on the
//!   market is still a whole number of nano-dollars, and an i64 holds
//!   nine billion dollars of them.

use crate::error::{AppError, AppResult, FieldErrorCode};

const NANOS_PER_USD: f64 = 1e9;
const MICROS_PER_USD: f64 = 1e6;

pub fn nanos_to_usd(nanos: i64) -> f64 {
    nanos as f64 / NANOS_PER_USD
}

pub fn micros_to_usd(micros: i64) -> f64 {
    micros as f64 / MICROS_PER_USD
}

/// A non-negative, finite dollar amount from a request, as nano-USD.
pub fn usd_to_nanos(usd: f64, field: &str) -> AppResult<i64> {
    to_units(usd, NANOS_PER_USD, field)
}

/// A per-million-token price from a request, as micro-USD.
pub fn usd_to_micros(usd: f64, field: &str) -> AppResult<i64> {
    to_units(usd, MICROS_PER_USD, field)
}

fn to_units(usd: f64, per_usd: f64, field: &str) -> AppResult<i64> {
    // Upper bound keeps the product inside i64.
    if !usd.is_finite() || !(0.0..=1e9).contains(&usd) {
        return Err(AppError::Validation(format!(
            "{field} must be a dollar amount between 0 and 1e9"
        ))
        .with_field(field, FieldErrorCode::Invalid));
    }
    Ok((usd * per_usd).round() as i64)
}

/// What a request cost, in nano-USD: tokens × (micro-USD per 1M tokens) is
/// micro-USD per million, i.e. pico-USD, so dividing by 1000 gives nano-USD.
pub fn cost_nanos(
    prompt_tokens: i64,
    completion_tokens: i64,
    input_price_micros: i64,
    output_price_micros: i64,
) -> i64 {
    let picos = i128::from(prompt_tokens) * i128::from(input_price_micros)
        + i128::from(completion_tokens) * i128::from(output_price_micros);
    i64::try_from(picos / 1000).unwrap_or(i64::MAX)
}

/// Serializes an optional nano-USD amount as optional dollars.
pub fn opt_usd(nanos: Option<i64>) -> Option<f64> {
    nanos.map(nanos_to_usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpt_4o_pricing_for_a_thousand_tokens_each_way() {
        // $2.50 in, $10.00 out per 1M tokens.
        let input = usd_to_micros(2.5, "p").unwrap();
        let output = usd_to_micros(10.0, "p").unwrap();
        let cost = cost_nanos(1000, 1000, input, output);
        assert_eq!(cost, 12_500_000); // $0.0125
        assert!((nanos_to_usd(cost) - 0.0125).abs() < 1e-12);
    }

    #[test]
    fn a_cheap_model_still_costs_whole_nanos() {
        // $0.075 / 1M, one token.
        assert_eq!(cost_nanos(1, 0, usd_to_micros(0.075, "p").unwrap(), 0), 75);
    }

    #[test]
    fn dollars_round_trip_through_nanos() {
        assert_eq!(usd_to_nanos(12.34, "b").unwrap(), 12_340_000_000);
        assert_eq!(nanos_to_usd(12_340_000_000), 12.34);
    }

    #[test]
    fn negative_infinite_and_absurd_amounts_are_refused() {
        for bad in [-1.0, f64::NAN, f64::INFINITY, 2e9] {
            assert!(usd_to_nanos(bad, "max_budget").is_err(), "{bad}");
        }
    }

    #[test]
    fn a_huge_request_saturates_instead_of_overflowing() {
        assert_eq!(cost_nanos(i64::MAX, i64::MAX, i64::MAX, i64::MAX), i64::MAX);
    }
}
