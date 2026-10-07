use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::db::{begin_write, is_unique_violation, Db, DbPool};
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::limits::{budget_period, next_reset, positive_limit};
use crate::models::list::{ListQuery, Paged};
use crate::models::money::{nanos_to_usd, opt_usd, usd_to_nanos};
use crate::models::required_name;
use crate::models::team::{
    CreateTeamRequest, Member, ModelRef, TeamDetail, TeamListQuery, TeamResponse, UpdateTeamRequest,
};
use crate::models::user::{validate_role, User};
use crate::services::users;

#[derive(sqlx::FromRow)]
struct TeamRow {
    id: i64,
    name: String,
    max_budget_nanos: Option<i64>,
    spend_nanos: i64,
    budget_duration: Option<String>,
    budget_reset_at: Option<DateTime<Utc>>,
    rpm_limit: Option<i64>,
    tpm_limit: Option<i64>,
    max_parallel_requests: Option<i64>,
    all_models: bool,
    created_at: DateTime<Utc>,
    member_count: i64,
    key_count: i64,
    my_role: Option<String>,
}

const TEAM_SELECT: &str = "
    SELECT t.id, t.name, t.max_budget_nanos, t.spend_nanos, t.budget_duration,
           t.budget_reset_at, t.rpm_limit, t.tpm_limit, t.max_parallel_requests, t.all_models,
           t.created_at,
           (SELECT COUNT(*) FROM team_members m WHERE m.team_id = t.id) AS member_count,
           (SELECT COUNT(*) FROM api_keys k WHERE k.team_id = t.id AND k.revoked_at IS NULL) AS key_count,
           (SELECT m.role FROM team_members m WHERE m.team_id = t.id AND m.user_id = $1) AS my_role
    FROM teams t
    WHERE ($2 OR EXISTS (SELECT 1 FROM team_members m WHERE m.team_id = t.id AND m.user_id = $1))";

/// The teams `user` can see (every team for a global admin, their own
/// otherwise), a page at a time.
pub async fn list_for(
    pool: &DbPool,
    user: &User,
    list: &ListQuery,
    filter: &TeamListQuery,
) -> AppResult<Paged<TeamResponse>> {
    let access = match filter.models.as_deref() {
        None | Some("") => 0,
        Some("all") => 1,
        Some("restricted") => 2,
        Some(other) => {
            return Err(AppError::Validation(format!(
                "models must be 'all' or 'restricted', not '{other}'"
            ))
            .with_field("models", FieldErrorCode::Invalid))
        }
    };
    let order = list.order_by(
        &[
            ("name", "LOWER(r.name)"),
            ("spend", "r.spend_nanos"),
            ("members", "r.member_count"),
            ("keys", "r.key_count"),
            ("created_at", "r.created_at"),
        ],
        "name",
        "r.id",
    )?;
    // The select's counts are columns of `r`, so they sort like any other.
    let from = format!(
        "FROM ({TEAM_SELECT}) AS r
         WHERE ($3 IS NULL OR LOWER(r.name) LIKE $3 ESCAPE '\\')
           AND ($4 = 0 OR ($4 = 1 AND r.all_models) OR ($4 = 2 AND NOT r.all_models))"
    );
    let search = list.search();
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) {from}")))
        .bind(user.id)
        .bind(user.is_admin())
        .bind(&search)
        .bind(access)
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as::<_, TeamRow>(sqlx::AssertSqlSafe(format!(
        "SELECT r.* {from} ORDER BY {order} LIMIT $5 OFFSET $6"
    )))
    .bind(user.id)
    .bind(user.is_admin())
    .bind(&search)
    .bind(access)
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    let mut models = team_models(pool).await?;
    let teams = rows
        .into_iter()
        .map(|row| {
            let team_models = models.remove(&row.id).unwrap_or_default();
            to_response(row, team_models)
        })
        .collect();
    Ok(list.paged(teams, total))
}

/// One team with its members. Access is checked by the caller.
pub async fn detail(pool: &DbPool, user: &User, id: i64) -> AppResult<TeamDetail> {
    let row =
        sqlx::query_as::<_, TeamRow>(sqlx::AssertSqlSafe(format!("{TEAM_SELECT} AND t.id = $3")))
            .bind(user.id)
            // Visibility was settled by the access check; read the row regardless.
            .bind(true)
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("team {id}")))?;
    let models = team_models(pool).await?.remove(&id).unwrap_or_default();
    let members = sqlx::query_as::<_, Member>(
        "SELECT m.user_id, u.email, m.role FROM team_members m
         JOIN users u ON u.id = m.user_id
         WHERE m.team_id = $1 ORDER BY u.email",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(TeamDetail {
        team: to_response(row, models),
        members,
        people: crate::services::people::list(pool, id).await?,
    })
}

pub async fn create(pool: &DbPool, user: &User, req: &CreateTeamRequest) -> AppResult<TeamDetail> {
    let name = required_name(&req.name, "name", 100)?;
    let budget = req
        .max_budget_usd
        .map(|usd| usd_to_nanos(usd, "max_budget_usd"))
        .transpose()?;
    let period = budget_period(req.budget_duration.as_deref())?;
    let now = Utc::now();

    let mut tx = begin_write(pool).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO teams (name, max_budget_nanos, spend_nanos, budget_duration, budget_reset_at,
                            rpm_limit, tpm_limit, all_models, created_at, max_parallel_requests)
         VALUES ($1, $2, 0, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(&name)
    .bind(budget)
    .bind(&period)
    .bind(period.as_deref().and_then(|p| next_reset(p, now)))
    .bind(positive_limit(req.rpm_limit, "rpm_limit")?)
    .bind(positive_limit(req.tpm_limit, "tpm_limit")?)
    .bind(req.all_models)
    .bind(now)
    .bind(positive_limit(
        req.max_parallel_requests,
        "max_parallel_requests",
    )?)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| name_conflict(e, &name))?;
    if !req.all_models {
        replace_models(&mut tx, id, &req.models).await?;
    }
    tx.commit().await?;
    detail(pool, user, id).await
}

pub async fn update(
    pool: &DbPool,
    user: &User,
    id: i64,
    req: &UpdateTeamRequest,
) -> AppResult<TeamDetail> {
    let mut tx = begin_write(pool).await?;
    if let Some(name) = &req.name {
        let name = required_name(name, "name", 100)?;
        sqlx::query("UPDATE teams SET name = $1 WHERE id = $2")
            .bind(&name)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| name_conflict(e, &name))?;
    }
    if let Some(budget) = req.max_budget_usd {
        let budget = budget
            .map(|usd| usd_to_nanos(usd, "max_budget_usd"))
            .transpose()?;
        sqlx::query("UPDATE teams SET max_budget_nanos = $1 WHERE id = $2")
            .bind(budget)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(period) = &req.budget_duration {
        let period = budget_period(period.as_deref())?;
        sqlx::query("UPDATE teams SET budget_duration = $1, budget_reset_at = $2 WHERE id = $3")
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
                "UPDATE teams SET {column} = $1 WHERE id = $2"
            )))
            .bind(positive_limit(limit, column)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
    }
    if let Some(all) = req.all_models {
        sqlx::query("UPDATE teams SET all_models = $1 WHERE id = $2")
            .bind(all)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if all {
            replace_models(&mut tx, id, &[]).await?;
        }
    }
    if let Some(models) = &req.models {
        replace_models(&mut tx, id, models).await?;
    }
    tx.commit().await?;
    detail(pool, user, id).await
}

pub async fn delete(pool: &DbPool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(AppError::NotFound(format!("team {id}")));
    }
    Ok(())
}

pub async fn add_member(pool: &DbPool, team_id: i64, email: &str, role: &str) -> AppResult<Member> {
    validate_role(role)?;
    let user = users::get_by_email(pool, email).await?.ok_or_else(|| {
        AppError::NotFound(format!("no user with email '{}'", email.trim()))
            .with_field("email", FieldErrorCode::Invalid)
    })?;
    sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(team_id)
        .bind(user.id)
        .bind(role)
        .execute(pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                AppError::Conflict(format!("{} is already in this team", user.email))
                    .with_field("email", FieldErrorCode::AlreadyExists)
            } else {
                e.into()
            }
        })?;
    Ok(Member {
        user_id: user.id,
        email: user.email,
        role: role.to_string(),
    })
}

pub async fn update_member(
    pool: &DbPool,
    team_id: i64,
    user_id: i64,
    role: &str,
) -> AppResult<Member> {
    validate_role(role)?;
    let updated =
        sqlx::query("UPDATE team_members SET role = $1 WHERE team_id = $2 AND user_id = $3")
            .bind(role)
            .bind(team_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected();
    if updated == 0 {
        return Err(AppError::NotFound(format!(
            "user {user_id} in team {team_id}"
        )));
    }
    let user = users::require(pool, user_id).await?;
    Ok(Member {
        user_id,
        email: user.email,
        role: role.to_string(),
    })
}

pub async fn remove_member(pool: &DbPool, team_id: i64, user_id: i64) -> AppResult<()> {
    let deleted = sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(AppError::NotFound(format!(
            "user {user_id} in team {team_id}"
        )));
    }
    Ok(())
}

/// The allowlist of every team, keyed by team id.
async fn team_models(pool: &DbPool) -> AppResult<HashMap<i64, Vec<ModelRef>>> {
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT tm.team_id, m.id, m.name FROM team_models tm
         JOIN models m ON m.id = tm.model_id ORDER BY m.name",
    )
    .fetch_all(pool)
    .await?;
    let mut by_team: HashMap<i64, Vec<ModelRef>> = HashMap::new();
    for (team_id, id, name) in rows {
        by_team
            .entry(team_id)
            .or_default()
            .push(ModelRef { id, name });
    }
    Ok(by_team)
}

/// Replaces a team's (or key's) model allowlist. Unknown ids are a 400 on
/// `models` rather than a foreign-key 500.
pub async fn replace_models(
    tx: &mut sqlx::Transaction<'_, Db>,
    team_id: i64,
    model_ids: &[i64],
) -> AppResult<()> {
    sqlx::query("DELETE FROM team_models WHERE team_id = $1")
        .bind(team_id)
        .execute(&mut **tx)
        .await?;
    for model_id in dedup(model_ids) {
        ensure_model_exists(tx, model_id).await?;
        sqlx::query("INSERT INTO team_models (team_id, model_id) VALUES ($1, $2)")
            .bind(team_id)
            .bind(model_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub async fn ensure_model_exists(
    tx: &mut sqlx::Transaction<'_, Db>,
    model_id: i64,
) -> AppResult<()> {
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM models WHERE id = $1")
        .bind(model_id)
        .fetch_optional(&mut **tx)
        .await?;
    found.map(|_| ()).ok_or_else(|| {
        AppError::Validation(format!("model {model_id} does not exist"))
            .with_field("models", FieldErrorCode::Invalid)
    })
}

pub fn dedup(ids: &[i64]) -> Vec<i64> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn to_response(row: TeamRow, models: Vec<ModelRef>) -> TeamResponse {
    TeamResponse {
        id: row.id,
        name: row.name,
        max_budget_usd: opt_usd(row.max_budget_nanos),
        spend_usd: nanos_to_usd(period_spend(row.spend_nanos, row.budget_reset_at)),
        budget_duration: row.budget_duration,
        budget_reset_at: row.budget_reset_at,
        rpm_limit: row.rpm_limit,
        tpm_limit: row.tpm_limit,
        max_parallel_requests: row.max_parallel_requests,
        created_at: row.created_at,
        member_count: row.member_count,
        key_count: row.key_count,
        all_models: row.all_models,
        models,
        my_role: row.my_role,
    }
}

/// Spend in the current period: a period that has ended resets on the next
/// request, so until then it reads as nothing spent.
pub fn period_spend(spend_nanos: i64, reset_at: Option<DateTime<Utc>>) -> i64 {
    if reset_at.is_some_and(|at| at <= Utc::now()) {
        0
    } else {
        spend_nanos
    }
}

fn name_conflict(error: sqlx::Error, name: &str) -> AppError {
    if is_unique_violation(&error) {
        AppError::Conflict(format!("a team named '{name}' already exists"))
            .with_field("name", FieldErrorCode::AlreadyExists)
    } else {
        error.into()
    }
}
