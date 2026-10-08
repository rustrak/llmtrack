use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::patch;

/// The dashboard's palette; a label is one of them.
pub const COLORS: [&str; 9] = [
    "gray", "red", "orange", "amber", "green", "teal", "blue", "violet", "pink",
];

/// Something an admin groups keys by: a client, a product, an environment.
#[derive(Debug, Serialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Active keys that carry it.
    pub key_count: i64,
}

/// A label as a key shows it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LabelRef {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLabelRequest {
    pub name: Option<String>,
    pub color: Option<String>,
    #[serde(default, deserialize_with = "patch")]
    pub description: Option<Option<String>>,
}
