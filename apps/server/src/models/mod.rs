pub mod invitation;
pub mod key;
pub mod limits;
pub mod list;
pub mod llm_model;
pub mod money;
pub mod person;
pub mod team;
pub mod user;

use serde::{Deserialize, Deserializer};

/// For PATCH bodies: a missing field is `None` (keep), an explicit `null` is
/// `Some(None)` (clear), a value is `Some(Some(v))` (set).
pub fn patch<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Trims a required name and checks its length.
pub fn required_name(value: &str, field: &str, max: usize) -> crate::error::AppResult<String> {
    use crate::error::{AppError, FieldErrorCode};
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{field} is required"))
            .with_field(field, FieldErrorCode::Required));
    }
    if value.chars().count() > max {
        return Err(
            AppError::Validation(format!("{field} is longer than {max} characters"))
                .with_field(field, FieldErrorCode::TooLong),
        );
    }
    Ok(value.to_string())
}
