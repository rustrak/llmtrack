use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::error::{AppError, AppResult, FieldErrorCode};

pub const ROLES: [&str; 2] = ["admin", "member"];
pub const MIN_PASSWORD_LEN: usize = 8;
/// Spend is billed in USD; the rest are display currencies.
pub const CURRENCIES: [&str; 2] = ["USD", "EUR"];

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// `admin` manages users, teams and models; `member` sees the teams they
    /// belong to.
    pub role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub name: Option<String>,
    /// BCP 47 tag the dashboard renders in; `None` follows the browser.
    pub language: Option<String>,
    /// IANA zone dates are shown in; `None` follows the browser.
    pub timezone: Option<String>,
    /// One of [`CURRENCIES`] money is shown in; `None` is USD.
    pub currency: Option<String>,
    /// Units of `currency` one US dollar buys, as the user typed it. Spend
    /// stays in USD; the dashboard converts with this.
    pub currency_rate: Option<f64>,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    pub fn verify_password(&self, password: &str) -> AppResult<bool> {
        let parsed = PasswordHash::new(&self.password_hash)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// `PATCH /auth/me`: a missing field is kept, `null` clears it.
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    #[serde(default, deserialize_with = "super::patch")]
    pub name: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::patch")]
    pub language: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::patch")]
    pub timezone: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::patch")]
    pub currency: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::patch")]
    pub currency_rate: Option<Option<f64>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    #[serde(default = "member")]
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
    pub is_active: Option<bool>,
    pub password: Option<String>,
}

fn member() -> String {
    "member".into()
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

pub fn hash_password(password: &str) -> AppResult<String> {
    Ok(Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string())
}

pub fn validate_password(password: &str, field: &str) -> AppResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        ))
        .with_field(field, FieldErrorCode::Invalid));
    }
    Ok(())
}

pub fn validate_role(role: &str) -> AppResult<()> {
    if !ROLES.contains(&role) {
        return Err(AppError::Validation(format!("unknown role '{role}'"))
            .with_field("role", FieldErrorCode::Invalid));
    }
    Ok(())
}

/// `es`, `en-GB`: dash-separated alphanumeric parts. Which tags the dashboard
/// can render is its business; this only keeps garbage out.
pub fn is_language_tag(tag: &str) -> bool {
    tag.len() <= 35
        && tag
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// `Europe/Madrid`, `UTC`, `Etc/GMT+1`: the shape of an IANA zone name.
pub fn is_time_zone_name(zone: &str) -> bool {
    zone.len() <= 64
        && zone.split('/').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_+-".contains(c))
        })
}

/// One `@`, a non-empty local part, a dotted domain. Deliverability is the
/// mail server's business.
pub fn validate_email(email: &str) -> AppResult<()> {
    let valid = match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && !domain.contains('@')
                && domain.split('.').count() >= 2
                && domain.split('.').all(|part| !part.is_empty())
        }
        None => false,
    };
    if !valid {
        return Err(
            AppError::Validation(format!("'{email}' is not an email address"))
                .with_field("email", FieldErrorCode::Invalid),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        for good in ["a@b.co", "first.last@sub.example.com"] {
            assert!(validate_email(good).is_ok(), "{good}");
        }
        for bad in ["", "a", "a@b", "@b.co", "a@@b.co", "a@b..co", "a@.co"] {
            assert!(validate_email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn email_normalizes_to_trimmed_lowercase() {
        assert_eq!(normalize_email("  Ada@Example.COM "), "ada@example.com");
    }

    #[test]
    fn password_roundtrips_through_argon2id() {
        let hash = hash_password("hunter42!").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        let user = User {
            id: 1,
            email: "a@b.co".into(),
            password_hash: hash,
            role: "member".into(),
            is_active: true,
            created_at: Utc::now(),
            last_login: None,
            name: None,
            language: None,
            timezone: None,
            currency: None,
            currency_rate: None,
        };
        assert!(user.verify_password("hunter42!").unwrap());
        assert!(!user.verify_password("hunter43!").unwrap());
    }

    #[test]
    fn language_tags_and_time_zones_are_shape_checked() {
        assert!(is_language_tag("es") && is_language_tag("en-GB"));
        assert!(!is_language_tag("en_GB") && !is_language_tag("") && !is_language_tag("es-"));
        assert!(is_time_zone_name("Europe/Madrid") && is_time_zone_name("Etc/GMT+1"));
        assert!(!is_time_zone_name("Europe/../etc") && !is_time_zone_name("a//b"));
    }

    #[test]
    fn short_passwords_and_unknown_roles_are_refused() {
        assert!(validate_password("1234567", "password").is_err());
        assert!(validate_password("12345678", "password").is_ok());
        assert!(validate_role("owner").is_err());
        assert!(validate_role("admin").is_ok());
    }
}
