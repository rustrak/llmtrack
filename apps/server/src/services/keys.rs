use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

use crate::crypto::generate_key;
use crate::db::{begin_write, Db, DbPool};
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::key::{
    CreateKeyRequest, CreatedKey, KeyListQuery, KeyResponse, UpdateKeyRequest,
};
use crate::models::limits::{budget_period, next_reset, positive_limit};
use crate::models::list::{ListQuery, Paged};
use crate::models::money::{nanos_to_usd, opt_usd, usd_to_nanos};
use crate::models::required_name;
use crate::models::team::ModelRef;
use crate::models::user::User;
use crate::services::access::{team_access, TeamAccess};
use crate::services::people;
use crate::services::teams::{dedup, ensure_model_exists, period_spend};

#[derive(sqlx::FromRow)]
struct KeyRow {
    id: i64,
    name: String,
    team_id: Option<i64>,
    team_name: Option<String>,
    user_id: Option<i64>,
    owner_email: Option<String>,
    person_id: Option<i64>,
    person_name: Option<String>,
    last4: String,
    max_budget_nanos: Option<i64>,
    spend_nanos: i64,
    budget_duration: Option<String>,
    budget_reset_at: Option<DateTime<Utc>>,
    rpm_limit: Option<i64>,
    tpm_limit: Option<i64>,
    max_parallel_requests: Option<i64>,
    blocked: bool,
    expires_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    created_by: Option<i64>,
    created_by_email: Option<String>,
}

const KEY_SELECT: &str = "
    SELECT k.id, k.name, k.team_id, t.name AS team_name, k.user_id, o.email AS owner_email,
           k.person_id, p.name AS person_name, k.last4, k.max_budget_nanos, k.spend_nanos, k.budget_duration, k.budget_reset_at,
           k.rpm_limit, k.tpm_limit, k.max_parallel_requests, k.blocked, k.expires_at, k.last_used_at, k.created_at,
           k.created_by, u.email AS created_by_email
    FROM api_keys k
    LEFT JOIN teams t ON t.id = k.team_id
    LEFT JOIN users o ON o.id = k.user_id
    LEFT JOIN people p ON p.id = k.person_id
    LEFT JOIN users u ON u.id = k.created_by
    WHERE k.revoked_at IS NULL";

/// Active keys `user` can see (every key for an admin; their teams' keys and
/// their own personal keys otherwise), a page at a time.
pub async fn list_for(
    pool: &DbPool,
    user: &User,
    list: &ListQuery,
    filter: &KeyListQuery,
) -> AppResult<Paged<KeyResponse>> {
    let status = match filter.status.as_deref() {
        None | Some("") => 0,
        Some("active") => 1,
        Some("blocked") => 2,
        Some("expired") => 3,
        Some(other) => {
            return Err(AppError::Validation(format!(
                "status must be 'active', 'blocked' or 'expired', not '{other}'"
            ))
            .with_field("status", FieldErrorCode::Invalid))
        }
    };
    let order = list.order_by(
        &[
            ("name", "LOWER(k.name)"),
            ("team", "LOWER(t.name)"),
            ("spend", "k.spend_nanos"),
            ("expires_at", "k.expires_at"),
            ("last_used_at", "k.last_used_at"),
            ("created_at", "k.created_at"),
        ],
        "-created_at",
        "k.id",
    )?;
    const WHERE: &str = "
           AND ($1 OR k.user_id = $2
                OR EXISTS (SELECT 1 FROM team_members m WHERE m.team_id = k.team_id AND m.user_id = $2))
           AND ($3 IS NULL OR k.team_id = $3)
           AND ($4 IS NULL OR LOWER(k.name) LIKE $4 ESCAPE '\\'
                OR LOWER(t.name) LIKE $4 ESCAPE '\\' OR LOWER(o.email) LIKE $4 ESCAPE '\\'
                OR LOWER(p.name) LIKE $4 ESCAPE '\\')
           AND ($5 = 0
                OR ($5 = 1 AND NOT k.blocked AND (k.expires_at IS NULL OR k.expires_at > $6))
                OR ($5 = 2 AND k.blocked)
                OR ($5 = 3 AND k.expires_at <= $6))";
    let search = list.search();
    let now = Utc::now();

    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM ({KEY_SELECT} {WHERE}) AS matching"
    )))
    .bind(user.is_admin())
    .bind(user.id)
    .bind(filter.team_id)
    .bind(&search)
    .bind(status)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query_as::<_, KeyRow>(sqlx::AssertSqlSafe(format!(
        "{KEY_SELECT} {WHERE} ORDER BY {order} LIMIT $7 OFFSET $8"
    )))
    .bind(user.is_admin())
    .bind(user.id)
    .bind(filter.team_id)
    .bind(&search)
    .bind(status)
    .bind(now)
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    let mut models = key_models(pool).await?;
    let keys = rows
        .into_iter()
        .map(|row| {
            let key_models = models.remove(&row.id).unwrap_or_default();
            to_response(row, key_models)
        })
        .collect();
    Ok(list.paged(keys, total))
}

/// One key, if `user` can see it.
pub async fn detail(pool: &DbPool, user: &User, id: i64) -> AppResult<KeyResponse> {
    let key = get(pool, id).await?;
    let visible = match key.team_id {
        _ if user.is_admin() => true,
        Some(team_id) => team_access(pool, user, team_id).await.is_ok(),
        None => key.user_id == Some(user.id),
    };
    if !visible {
        return Err(AppError::NotFound(format!("key {id}")));
    }
    Ok(key)
}

pub async fn get(pool: &DbPool, id: i64) -> AppResult<KeyResponse> {
    let row =
        sqlx::query_as::<_, KeyRow>(sqlx::AssertSqlSafe(format!("{KEY_SELECT} AND k.id = $1")))
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("key {id}")))?;
    let models = key_models(pool).await?.remove(&id).unwrap_or_default();
    Ok(to_response(row, models))
}

pub async fn create(pool: &DbPool, user: &User, req: &CreateKeyRequest) -> AppResult<CreatedKey> {
    if let Some(team_id) = req.team_id {
        team_access(pool, user, team_id).await?;
    }
    let name = required_name(&req.name, "name", 100)?;
    people::check_assignable(pool, req.team_id, req.person_id).await?;
    let budget = req
        .max_budget_usd
        .map(|usd| usd_to_nanos(usd, "max_budget_usd"))
        .transpose()?;
    let period = budget_period(req.budget_duration.as_deref())?;
    validate_expiry(req.expires_at)?;
    let key = generate_key();
    let now = Utc::now();

    let mut tx = begin_write(pool).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO api_keys (team_id, user_id, name, key_hash, last4, created_by,
                               max_budget_nanos, spend_nanos, budget_duration, budget_reset_at,
                               rpm_limit, tpm_limit, blocked, expires_at, created_at,
                               max_parallel_requests, person_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $8, $9, $10, $11, FALSE, $12, $13, $14, $15)
         RETURNING id",
    )
    .bind(req.team_id)
    .bind(req.team_id.is_none().then_some(user.id))
    .bind(&name)
    .bind(&key.hash)
    .bind(&key.last4)
    .bind(user.id)
    .bind(budget)
    .bind(&period)
    .bind(period.as_deref().and_then(|p| next_reset(p, now)))
    .bind(positive_limit(req.rpm_limit, "rpm_limit")?)
    .bind(positive_limit(req.tpm_limit, "tpm_limit")?)
    .bind(req.expires_at)
    .bind(now)
    .bind(positive_limit(
        req.max_parallel_requests,
        "max_parallel_requests",
    )?)
    .bind(req.person_id)
    .fetch_one(&mut *tx)
    .await?;
    replace_models(&mut tx, id, req.team_id, &req.models).await?;
    tx.commit().await?;
    Ok(CreatedKey {
        info: get(pool, id).await?,
        key: key.raw,
    })
}

pub async fn update(
    pool: &DbPool,
    user: &User,
    id: i64,
    req: &UpdateKeyRequest,
) -> AppResult<KeyResponse> {
    let key = authorize(pool, user, id).await?;
    if let Some(person_id) = req.person_id {
        people::check_assignable(pool, key.team_id, person_id).await?;
    }
    let mut tx = begin_write(pool).await?;
    if let Some(person_id) = req.person_id {
        sqlx::query("UPDATE api_keys SET person_id = $1 WHERE id = $2")
            .bind(person_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(name) = &req.name {
        sqlx::query("UPDATE api_keys SET name = $1 WHERE id = $2")
            .bind(required_name(name, "name", 100)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(budget) = req.max_budget_usd {
        let budget = budget
            .map(|usd| usd_to_nanos(usd, "max_budget_usd"))
            .transpose()?;
        sqlx::query("UPDATE api_keys SET max_budget_nanos = $1 WHERE id = $2")
            .bind(budget)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(period) = &req.budget_duration {
        let period = budget_period(period.as_deref())?;
        sqlx::query("UPDATE api_keys SET budget_duration = $1, budget_reset_at = $2 WHERE id = $3")
            .bind(&period)
            .bind(period.as_deref().and_then(|p| next_reset(p, Utc::now())))
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    for (column, limit) in [
        ("rpm_limit", req.rpm_limit),
        ("tpm_limit", req.tpm_limit),
        ("max_parallel_requests", req.max_parallel_requests),
    ] {
        if let Some(limit) = limit {
            // `column` is one of the two literals above.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE api_keys SET {column} = $1 WHERE id = $2"
            )))
            .bind(positive_limit(limit, column)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
    }
    if let Some(expires_at) = req.expires_at {
        validate_expiry(expires_at)?;
        sqlx::query("UPDATE api_keys SET expires_at = $1 WHERE id = $2")
            .bind(expires_at)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(models) = &req.models {
        replace_models(&mut tx, id, key.team_id, models).await?;
    }
    tx.commit().await?;
    get(pool, id).await
}

/// Blocks or unblocks a key. Unlike revoking, this can be undone.
pub async fn set_blocked(
    pool: &DbPool,
    user: &User,
    id: i64,
    blocked: bool,
) -> AppResult<KeyResponse> {
    authorize(pool, user, id).await?;
    sqlx::query("UPDATE api_keys SET blocked = $1 WHERE id = $2")
        .bind(blocked)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

/// Gives a key a new secret. Everything else (settings, spend, history)
/// stays; the old secret stops working.
pub async fn regenerate(pool: &DbPool, user: &User, id: i64) -> AppResult<CreatedKey> {
    authorize(pool, user, id).await?;
    let key = generate_key();
    sqlx::query("UPDATE api_keys SET key_hash = $1, last4 = $2 WHERE id = $3")
        .bind(&key.hash)
        .bind(&key.last4)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(CreatedKey {
        info: get(pool, id).await?,
        key: key.raw,
    })
}

/// Revokes a key. The row stays, so its history keeps a name.
pub async fn revoke(pool: &DbPool, user: &User, id: i64) -> AppResult<()> {
    authorize(pool, user, id).await?;
    sqlx::query("UPDATE api_keys SET revoked_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Loads an active key `user` may change: a team manager may change any key
/// of the team, a member only the keys they created, and a personal key
/// only its owner (or a global admin).
pub async fn authorize(pool: &DbPool, user: &User, id: i64) -> AppResult<KeyResponse> {
    let key = detail(pool, user, id).await?;
    let Some(team_id) = key.team_id else {
        return Ok(key);
    };
    let access = team_access(pool, user, team_id).await?;
    if access == TeamAccess::Member && key.created_by != Some(user.id) {
        return Err(AppError::Forbidden(
            "only team admins can change keys they did not create".into(),
        ));
    }
    Ok(key)
}

/// Replaces a key's model allowlist, which must sit inside its team's.
async fn replace_models(
    tx: &mut sqlx::Transaction<'_, Db>,
    key_id: i64,
    team_id: Option<i64>,
    model_ids: &[i64],
) -> AppResult<()> {
    // `None`: anything goes (a personal key, or a team with every model).
    // By name: any deployment of a name the team may call will do.
    let team_allows: Option<HashSet<String>> = match team_id {
        None => None,
        Some(team_id) => {
            let all: bool = sqlx::query_scalar("SELECT all_models FROM teams WHERE id = $1")
                .bind(team_id)
                .fetch_one(&mut **tx)
                .await?;
            if all {
                None
            } else {
                Some(
                    sqlx::query_scalar::<_, String>(
                        "SELECT m.name FROM team_models tm JOIN models m ON m.id = tm.model_id
                         WHERE tm.team_id = $1",
                    )
                    .bind(team_id)
                    .fetch_all(&mut **tx)
                    .await?
                    .into_iter()
                    .collect(),
                )
            }
        }
    };
    sqlx::query("DELETE FROM key_models WHERE key_id = $1")
        .bind(key_id)
        .execute(&mut **tx)
        .await?;
    for model_id in dedup(model_ids) {
        ensure_model_exists(tx, model_id).await?;
        let name: String = sqlx::query_scalar("SELECT name FROM models WHERE id = $1")
            .bind(model_id)
            .fetch_one(&mut **tx)
            .await?;
        if team_allows
            .as_ref()
            .is_some_and(|names| !names.contains(&name))
        {
            return Err(AppError::Validation(format!(
                "model {model_id} is not allowed for this team"
            ))
            .with_field("models", FieldErrorCode::Invalid));
        }
        sqlx::query("INSERT INTO key_models (key_id, model_id) VALUES ($1, $2)")
            .bind(key_id)
            .bind(model_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn validate_expiry(expires_at: Option<DateTime<Utc>>) -> AppResult<()> {
    if expires_at.is_some_and(|at| at <= Utc::now()) {
        return Err(
            AppError::Validation("expires_at must be in the future".into())
                .with_field("expires_at", FieldErrorCode::Invalid),
        );
    }
    Ok(())
}

async fn key_models(pool: &DbPool) -> AppResult<HashMap<i64, Vec<ModelRef>>> {
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT km.key_id, m.id, m.name FROM key_models km
         JOIN models m ON m.id = km.model_id ORDER BY m.name",
    )
    .fetch_all(pool)
    .await?;
    let mut by_key: HashMap<i64, Vec<ModelRef>> = HashMap::new();
    for (key_id, id, name) in rows {
        by_key
            .entry(key_id)
            .or_default()
            .push(ModelRef { id, name });
    }
    Ok(by_key)
}

fn to_response(row: KeyRow, models: Vec<ModelRef>) -> KeyResponse {
    KeyResponse {
        id: row.id,
        name: row.name,
        team_id: row.team_id,
        team_name: row.team_name,
        user_id: row.user_id,
        owner_email: row.owner_email,
        person_id: row.person_id,
        person_name: row.person_name,
        key_hint: format!("sk-...{}", row.last4),
        models,
        max_budget_usd: opt_usd(row.max_budget_nanos),
        spend_usd: nanos_to_usd(period_spend(row.spend_nanos, row.budget_reset_at)),
        budget_duration: row.budget_duration,
        budget_reset_at: row.budget_reset_at,
        rpm_limit: row.rpm_limit,
        tpm_limit: row.tpm_limit,
        max_parallel_requests: row.max_parallel_requests,
        blocked: row.blocked,
        expires_at: row.expires_at,
        last_used_at: row.last_used_at,
        created_at: row.created_at,
        created_by: row.created_by,
        created_by_email: row.created_by_email,
    }
}
