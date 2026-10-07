use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// `pending`, `accepted` or `revoked`. An expired invitation is a pending
/// one past `expires_at`.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Invitation {
    /// Shared in the link, so it is the one secret here and stays visible
    /// to admins: the dashboard rebuilds the link to copy it again.
    pub token: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    #[serde(skip)]
    pub invited_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    pub accepted_at: Option<DateTime<Utc>>,
}

impl Invitation {
    pub fn is_acceptable(&self, now: DateTime<Utc>) -> bool {
        self.status == "pending" && self.expires_at > now
    }
}

/// What the public invite page may know: no token.
#[derive(Debug, Serialize)]
pub struct InvitationInfo {
    pub email: String,
    pub role: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    #[serde(default = "member")]
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInvitationRequest {
    pub token: String,
    pub password: String,
}

fn member() -> String {
    "member".into()
}
