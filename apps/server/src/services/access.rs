//! Who may do what to a team.
//!
//! Global admins may do everything. Inside a team, an `admin` manages its
//! members and every key; a `member` sees the team and manages the keys they
//! created. Anyone else gets a 404, not a 403: a team you are not in does not
//! exist as far as you can tell.

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::user::User;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamAccess {
    GlobalAdmin,
    TeamAdmin,
    Member,
}

impl TeamAccess {
    pub fn can_manage(self) -> bool {
        matches!(self, TeamAccess::GlobalAdmin | TeamAccess::TeamAdmin)
    }
}

/// The membership role, if any, of `user_id` in `team_id`.
pub async fn membership(pool: &DbPool, team_id: i64, user_id: i64) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2")
            .bind(team_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
    )
}

/// What `user` may do in `team_id`. Errors with 404 when the team does not
/// exist or the user cannot see it.
pub async fn team_access(pool: &DbPool, user: &User, team_id: i64) -> AppResult<TeamAccess> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM teams WHERE id = $1")
        .bind(team_id)
        .fetch_optional(pool)
        .await?;
    let not_found = || AppError::NotFound(format!("team {team_id}"));
    exists.ok_or_else(not_found)?;
    if user.is_admin() {
        return Ok(TeamAccess::GlobalAdmin);
    }
    match membership(pool, team_id, user.id).await?.as_deref() {
        Some("admin") => Ok(TeamAccess::TeamAdmin),
        Some(_) => Ok(TeamAccess::Member),
        None => Err(not_found()),
    }
}

/// Like [`team_access`], but only admins of the team pass.
pub async fn require_team_manager(
    pool: &DbPool,
    user: &User,
    team_id: i64,
) -> AppResult<TeamAccess> {
    let access = team_access(pool, user, team_id).await?;
    if !access.can_manage() {
        return Err(AppError::Forbidden("only team admins can do this".into()));
    }
    Ok(access)
}
