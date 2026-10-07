//! What keys and teams share: a budget that resets every period, and
//! per-minute request and token limits. Periods are strings:
//! `30s`, `30m`, `24h`, `7d`, `1mo`.

use chrono::{DateTime, Months, TimeDelta, Utc};

use crate::error::{AppError, AppResult, FieldErrorCode};

/// When a budget with this period, starting at `from`, resets next.
pub fn next_reset(period: &str, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if let Some(n) = period.strip_suffix("mo") {
        return from.checked_add_months(Months::new(n.parse().ok().filter(|n| *n > 0)?));
    }
    let unit = period.chars().last()?;
    let n: i64 = period[..period.len() - unit.len_utf8()]
        .parse()
        .ok()
        .filter(|n| *n > 0)?;
    let step = match unit {
        's' => TimeDelta::try_seconds(n),
        'm' => TimeDelta::try_minutes(n),
        'h' => TimeDelta::try_hours(n),
        'd' => TimeDelta::try_days(n),
        _ => None,
    }?;
    from.checked_add_signed(step)
}

/// A period from a request: trimmed, empty means none, unknown is a 400.
pub fn budget_period(period: Option<&str>) -> AppResult<Option<String>> {
    let Some(period) = period.map(str::trim).filter(|p| !p.is_empty()) else {
        return Ok(None);
    };
    if next_reset(period, Utc::now()).is_none() {
        return Err(AppError::Validation(format!(
            "budget_duration '{period}' is not a period like 1h, 1d, 7d or 1mo"
        ))
        .with_field("budget_duration", FieldErrorCode::Invalid));
    }
    Ok(Some(period.to_string()))
}

/// A limit from a request (per minute, or at once): at least 1 when set.
pub fn positive_limit(limit: Option<i64>, field: &str) -> AppResult<Option<i64>> {
    if limit.is_some_and(|n| n < 1) {
        return Err(AppError::Validation(format!("{field} must be at least 1"))
            .with_field(field, FieldErrorCode::Invalid));
    }
    Ok(limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_parse() {
        let at = DateTime::parse_from_rfc3339("2026-01-31T10:00:00Z")
            .unwrap()
            .to_utc();
        let next = |p| next_reset(p, at).map(|t| t.to_rfc3339());
        assert_eq!(next("30s").unwrap(), "2026-01-31T10:00:30+00:00");
        assert_eq!(next("1h").unwrap(), "2026-01-31T11:00:00+00:00");
        assert_eq!(next("7d").unwrap(), "2026-02-07T10:00:00+00:00");
        assert_eq!(next("1mo").unwrap(), "2026-02-28T10:00:00+00:00");
        for bad in ["", "d", "0d", "-1d", "1w", "fortnight", "1.5h", "1é"] {
            assert!(next(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn empty_periods_and_limits_are_none() {
        assert_eq!(budget_period(Some(" ")).unwrap(), None);
        assert!(budget_period(Some("1y")).is_err());
        assert!(positive_limit(Some(0), "rpm_limit").is_err());
        assert_eq!(positive_limit(Some(5), "rpm_limit").unwrap(), Some(5));
    }
}
