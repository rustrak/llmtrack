use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::patch;

#[derive(Debug, Clone, FromRow)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub max_budget_nanos: Option<i64>,
    pub spend_nanos: i64,
    pub created_at: DateTime<Utc>,
}

/// A model as other resources reference it.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ModelRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TeamResponse {
    pub id: i64,
    pub name: String,
    pub max_budget_usd: Option<f64>,
    /// Spend in the current budget period.
    pub spend_usd: f64,
    pub budget_duration: Option<String>,
    pub budget_reset_at: Option<DateTime<Utc>>,
    pub rpm_limit: Option<i64>,
    pub tpm_limit: Option<i64>,
    pub max_parallel_requests: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub member_count: i64,
    pub key_count: i64,
    /// Every model on the proxy, now and later; `models` is then empty.
    pub all_models: bool,
    /// The models the team may call when not `all_models`; empty means none.
    pub models: Vec<ModelRef>,
    /// The viewer's role in this team; `None` for a global admin outside it.
    pub my_role: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Member {
    pub user_id: i64,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct TeamDetail {
    #[serde(flatten)]
    pub team: TeamResponse,
    pub members: Vec<Member>,
    /// Who spends through the team's keys without a dashboard account.
    pub people: Vec<super::person::Person>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub max_budget_usd: Option<f64>,
    pub budget_duration: Option<String>,
    pub rpm_limit: Option<i64>,
    pub tpm_limit: Option<i64>,
    pub max_parallel_requests: Option<i64>,
    #[serde(default)]
    pub all_models: bool,
    #[serde(default)]
    pub models: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamRequest {
    pub name: Option<String>,
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
    pub all_models: Option<bool>,
    pub models: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub email: String,
    #[serde(default = "member")]
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
}

fn member() -> String {
    "member".into()
}

#[derive(Debug, Deserialize)]
pub struct TeamListQuery {
    /// `all`: teams that may call every model; `restricted`: the others.
    pub models: Option<String>,
}
