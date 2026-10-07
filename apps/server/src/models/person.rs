use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::patch;

/// Someone who spends through a team's keys without a dashboard account.
#[derive(Debug, Serialize)]
pub struct Person {
    pub id: i64,
    /// `None` until they are placed in a team; only global admins see them then.
    pub team_id: Option<i64>,
    pub team_name: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Active keys assigned to them.
    pub key_count: i64,
    /// Everything their keys have spent while assigned to them.
    pub spend_usd: f64,
}

#[derive(Debug, Deserialize)]
pub struct PersonListQuery {
    pub team_id: Option<i64>,
    /// Only people without a team.
    #[serde(default)]
    pub unassigned: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreatePersonRequest {
    pub name: String,
    pub email: Option<String>,
    pub team_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePersonRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "patch")]
    pub email: Option<Option<String>>,
    /// Moves them to a team, or (`null`) out of theirs. Their keys stay with
    /// the old team, assigned to no one.
    #[serde(default, deserialize_with = "patch")]
    pub team_id: Option<Option<i64>>,
}
