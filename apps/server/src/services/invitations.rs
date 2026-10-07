//! Invitations: an admin invites an email, shares the link by hand, and the
//! invitee becomes a user by setting a password. No email is sent.

use chrono::{Duration, Utc};
use rand::RngExt;

use crate::db::{begin_write, DbPool};
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::invitation::Invitation;
use crate::models::user::{
    normalize_email, validate_email, validate_password, validate_role, User,
};
use crate::services::users;

const TTL_HOURS: i64 = 48;

const COLUMNS: &str = "token, email, role, status, expires_at, invited_by, created_at, accepted_at";

fn taken(message: String) -> AppError {
    AppError::Conflict(message).with_field("email", FieldErrorCode::AlreadyExists)
}

pub async fn create(
    pool: &DbPool,
    email: &str,
    role: &str,
    invited_by: i64,
) -> AppResult<Invitation> {
    validate_role(role)?;
    let email = normalize_email(email);
    validate_email(&email)?;
    if users::get_by_email(pool, &email).await?.is_some() {
        return Err(taken(format!("a user with email '{email}' already exists")));
    }
    let pending: Option<String> =
        sqlx::query_scalar("SELECT token FROM invitations WHERE email = $1 AND status = 'pending'")
            .bind(&email)
            .fetch_optional(pool)
            .await?;
    if pending.is_some() {
        return Err(taken(format!("'{email}' already has a pending invitation")));
    }
    let now = Utc::now();
    Ok(sqlx::query_as::<_, Invitation>(sqlx::AssertSqlSafe(format!(
        "INSERT INTO invitations ({COLUMNS})
         VALUES ($1, $2, $3, 'pending', $4, $5, $6, NULL) RETURNING {COLUMNS}"
    )))
    .bind(hex::encode(rand::rng().random::<[u8; 20]>()))
    .bind(&email)
    .bind(role)
    .bind(now + Duration::hours(TTL_HOURS))
    .bind(invited_by)
    .bind(now)
    .fetch_one(pool)
    .await?)
}

/// Every invitation, newest first.
pub async fn list(pool: &DbPool) -> AppResult<Vec<Invitation>> {
    Ok(sqlx::query_as::<_, Invitation>(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM invitations ORDER BY created_at DESC"
    )))
    .fetch_all(pool)
    .await?)
}

/// A pending, unexpired invitation: 404 when unknown, 400 when used up.
pub async fn acceptable(pool: &DbPool, token: &str) -> AppResult<Invitation> {
    let invitation = sqlx::query_as::<_, Invitation>(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM invitations WHERE token = $1"
    )))
    .bind(token)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("invitation".into()))?;
    if !invitation.is_acceptable(Utc::now()) {
        return Err(AppError::Validation(
            "this invitation has expired or was already used".into(),
        ));
    }
    Ok(invitation)
}

/// Creates the invited user with `password`. Email and role come from the
/// invitation, never from the request.
pub async fn accept(pool: &DbPool, token: &str, password: &str) -> AppResult<User> {
    let invitation = acceptable(pool, token).await.map_err(|e| match e {
        AppError::NotFound(_) => AppError::Validation("invalid invitation".into()),
        e => e,
    })?;
    validate_password(password, "password")?;
    let mut tx = begin_write(pool).await?;
    let used = sqlx::query(
        "UPDATE invitations SET status = 'accepted', accepted_at = $1
         WHERE token = $2 AND status = 'pending'",
    )
    .bind(Utc::now())
    .bind(token)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if used == 0 {
        return Err(AppError::Validation(
            "this invitation was already used".into(),
        ));
    }
    let user = users::create(&mut *tx, &invitation.email, password, &invitation.role).await?;
    tx.commit().await?;
    Ok(user)
}

pub async fn revoke(pool: &DbPool, token: &str) -> AppResult<()> {
    let revoked = sqlx::query(
        "UPDATE invitations SET status = 'revoked' WHERE token = $1 AND status = 'pending'",
    )
    .bind(token)
    .execute(pool)
    .await?
    .rows_affected();
    if revoked == 0 {
        return Err(AppError::NotFound("pending invitation".into()));
    }
    Ok(())
}
