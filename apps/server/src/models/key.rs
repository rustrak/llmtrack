use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::patch;
use super::team::ModelRef;

#[derive(Debug, Serialize)]
pub struct KeyResponse {
    pub id: i64,
    pub name: String,
    /// A team key bills to its team; a personal key (no team) to its user.
    pub team_id: Option<i64>,
    pub team_name: Option<String>,
    pub user_id: Option<i64>,
    pub owner_email: Option<String>,
    /// The team's person the key is for, if any.
    pub person_id: Option<i64>,
    pub person_name: Option<String>,
    /// `sk-...abcd`: enough to recognise a key, useless to anyone holding it.
    pub key_hint: String,
    /// Empty means every model the team (or, for a personal key, the proxy) has.
    pub models: Vec<ModelRef>,
    pub max_budget_usd: Option<f64>,
    /// Spend in the current budget period.
    pub spend_usd: f64,
    pub budget_duration: Option<String>,
    pub budget_reset_at: Option<DateTime<Utc>>,
    pub rpm_limit: Option<i64>,
    pub tpm_limit: Option<i64>,
    pub max_parallel_requests: Option<i64>,
    pub blocked: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<i64>,
    pub created_by_email: Option<String>,
}

/// Returned once, at creation or regeneration: the only time the raw key
/// leaves the server.
#[derive(Debug, Serialize)]
pub struct CreatedKey {
    #[serde(flatten)]
    pub info: KeyResponse,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    /// `None` makes a personal key owned by whoever creates it.
    pub team_id: Option<i64>,
    /// One of the team's people; team keys only.
    pub person_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub models: Vec<i64>,
    pub max_budget_usd: Option<f64>,
    pub budget_duration: Option<String>,
    pub rpm_limit: Option<i64>,
    pub tpm_limit: Option<i64>,
    pub max_parallel_requests: Option<i64>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateKeyRequest {
    pub name: Option<String>,
    pub models: Option<Vec<i64>>,
    #[serde(default, deserialize_with = "patch")]
    pub person_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch")]
    pub max_budget_usd: Option<Option<f64>>,
    #[serde(default, deserialize_with = "patch")]
    pub budget_duration: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch")]
    pub rpm_limit: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch")]
    pub tpm_limit: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch")]
    pub max_parallel_requests: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch")]
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Deserialize)]
pub struct KeyListQuery {
    pub team_id: Option<i64>,
    /// `active`, `blocked` or `expired`.
    pub status: Option<String>,
}
