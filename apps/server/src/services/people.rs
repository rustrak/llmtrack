//! People: who spends through a team's keys without signing in. Created on
//! their own, placed in at most one team. A team's admins manage its people;
//! people without a team are for global admins only.

use chrono::{DateTime, Utc};

use crate::db::{is_unique_violation, DbPool};
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::list::{ListQuery, Paged};
use crate::models::money::nanos_to_usd;
use crate::models::person::{CreatePersonRequest, Person, PersonListQuery, UpdatePersonRequest};
use crate::models::required_name;
use crate::models::user::{normalize_email, validate_email, User};
use crate::services::access::require_team_manager;

#[derive(sqlx::FromRow)]
struct PersonRow {
    id: i64,
    team_id: Option<i64>,
    team_name: Option<String>,
    name: String,
    email: Option<String>,
    created_at: DateTime<Utc>,
    key_count: i64,
    spend_nanos: i64,
}

const SELECT: &str = "
    SELECT p.id, p.team_id, t.name AS team_name, p.name, p.email, p.created_at,
           (SELECT COUNT(*) FROM api_keys k
            WHERE k.person_id = p.id AND k.revoked_at IS NULL) AS key_count,
           CAST(COALESCE((SELECT SUM(u.cost_nanos) FROM usage_daily u
                          WHERE u.person_id = p.id), 0) AS BIGINT) AS spend_nanos
    FROM people p
    LEFT JOIN teams t ON t.id = p.team_id";

/// One team's people. Access to the team is checked by the caller.
pub async fn list(pool: &DbPool, team_id: i64) -> AppResult<Vec<Person>> {
    let rows = sqlx::query_as::<_, PersonRow>(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE p.team_id = $1 ORDER BY LOWER(p.name)"
    )))
    .bind(team_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(to_person).collect())
}

/// The people of every team `user` can see (everyone, teamless included,
/// for an admin), a page at a time.
pub async fn list_for(
    pool: &DbPool,
    user: &User,
    list: &ListQuery,
    filter: &PersonListQuery,
) -> AppResult<Paged<Person>> {
    let order = list.order_by(
        &[
            ("name", "LOWER(r.name)"),
            ("team", "LOWER(r.team_name)"),
            ("keys", "r.key_count"),
            ("spend", "r.spend_nanos"),
            ("created_at", "r.created_at"),
        ],
        "name",
        "r.id",
    )?;
    let from = format!(
        "FROM ({SELECT}) AS r
         WHERE ($1 OR EXISTS (SELECT 1 FROM team_members m
                              WHERE m.team_id = r.team_id AND m.user_id = $2))
           AND ($3 IS NULL OR r.team_id = $3)
           AND (NOT $4 OR r.team_id IS NULL)
           AND ($5 IS NULL OR LOWER(r.name) LIKE $5 ESCAPE '\\'
                OR LOWER(r.email) LIKE $5 ESCAPE '\\'
                OR LOWER(r.team_name) LIKE $5 ESCAPE '\\')"
    );
    let search = list.search();
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) {from}")))
        .bind(user.is_admin())
        .bind(user.id)
        .bind(filter.team_id)
        .bind(filter.unassigned)
        .bind(&search)
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as::<_, PersonRow>(sqlx::AssertSqlSafe(format!(
        "SELECT r.* {from} ORDER BY {order} LIMIT $6 OFFSET $7"
    )))
    .bind(user.is_admin())
    .bind(user.id)
    .bind(filter.team_id)
    .bind(filter.unassigned)
    .bind(&search)
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    Ok(list.paged(rows.into_iter().map(to_person).collect(), total))
}

async fn get(pool: &DbPool, id: i64) -> AppResult<Person> {
    sqlx::query_as::<_, PersonRow>(sqlx::AssertSqlSafe(format!("{SELECT} WHERE p.id = $1")))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(to_person)
        .ok_or_else(|| AppError::NotFound(format!("person {id}")))
}

/// Who manages a person, or a team someone is being placed in: its admins,
/// or for no team, global admins only.
async fn require_manager(pool: &DbPool, user: &User, team_id: Option<i64>) -> AppResult<()> {
    match team_id {
        Some(team_id) => require_team_manager(pool, user, team_id).await.map(|_| ()),
        None if user.is_admin() => Ok(()),
        None => Err(AppError::Forbidden(
            "only admins manage people without a team".into(),
        )),
    }
}

/// A person `user` may change: unseen people are a 404, seen ones a 403.
async fn managed(pool: &DbPool, user: &User, id: i64) -> AppResult<Person> {
    let person = get(pool, id).await?;
    if person.team_id.is_none() && !user.is_admin() {
        return Err(AppError::NotFound(format!("person {id}")));
    }
    require_manager(pool, user, person.team_id).await?;
    Ok(person)
}

pub async fn create(pool: &DbPool, user: &User, req: &CreatePersonRequest) -> AppResult<Person> {
    require_manager(pool, user, req.team_id).await?;
    let name = required_name(&req.name, "name", 100)?;
    let email = email(req.email.as_deref())?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO people (team_id, name, email, created_at) VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(req.team_id)
    .bind(&name)
    .bind(email)
    .bind(Utc::now())
    .fetch_one(pool)
    .await
    .map_err(taken)?;
    get(pool, id).await
}

pub async fn update(
    pool: &DbPool,
    user: &User,
    id: i64,
    req: &UpdatePersonRequest,
) -> AppResult<Person> {
    let person = managed(pool, user, id).await?;
    if let Some(name) = &req.name {
        sqlx::query("UPDATE people SET name = $1 WHERE id = $2")
            .bind(required_name(name, "name", 100)?)
            .bind(id)
            .execute(pool)
            .await
            .map_err(taken)?;
    }
    if let Some(value) = &req.email {
        sqlx::query("UPDATE people SET email = $1 WHERE id = $2")
            .bind(email(value.as_deref())?)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(team_id) = req.team_id.filter(|t| *t != person.team_id) {
        require_manager(pool, user, team_id).await?;
        sqlx::query("UPDATE people SET team_id = $1 WHERE id = $2")
            .bind(team_id)
            .bind(id)
            .execute(pool)
            .await
            .map_err(taken)?;
        // Their keys belong to the old team, which keeps paying for them.
        sqlx::query("UPDATE api_keys SET person_id = NULL WHERE person_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    get(pool, id).await
}

/// Their keys stay with the team, unassigned; their history keeps the id.
pub async fn delete(pool: &DbPool, user: &User, id: i64) -> AppResult<()> {
    managed(pool, user, id).await?;
    sqlx::query("DELETE FROM people WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// A key may be assigned only to someone of its own team.
pub async fn check_assignable(
    pool: &DbPool,
    team_id: Option<i64>,
    person_id: Option<i64>,
) -> AppResult<()> {
    let Some(person_id) = person_id else {
        return Ok(());
    };
    let person_team: Option<Option<i64>> =
        sqlx::query_scalar("SELECT team_id FROM people WHERE id = $1")
            .bind(person_id)
            .fetch_optional(pool)
            .await?;
    if team_id.is_none() || person_team.flatten() != team_id {
        return Err(
            AppError::Validation(format!("person {person_id} is not in this key's team"))
                .with_field("person_id", FieldErrorCode::Invalid),
        );
    }
    Ok(())
}

fn email(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(email) = value.map(normalize_email).filter(|e| !e.is_empty()) else {
        return Ok(None);
    };
    validate_email(&email)?;
    Ok(Some(email))
}

fn taken(error: sqlx::Error) -> AppError {
    if is_unique_violation(&error) {
        return AppError::Validation("someone in this team already has that name".into())
            .with_field("name", FieldErrorCode::AlreadyExists);
    }
    error.into()
}

fn to_person(row: PersonRow) -> Person {
    Person {
        id: row.id,
        team_id: row.team_id,
        team_name: row.team_name,
        name: row.name,
        email: row.email,
        created_at: row.created_at,
        key_count: row.key_count,
        spend_usd: nanos_to_usd(row.spend_nanos),
    }
}
