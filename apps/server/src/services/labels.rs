//! Labels: defined by admins, assigned to keys (several each). Usage is
//! filtered by the keys that carry a label now (`services/usage.rs`).

use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::db::{is_unique_violation, Db, DbPool};
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::label::{CreateLabelRequest, Label, LabelRef, UpdateLabelRequest, COLORS};
use crate::models::list::{ListQuery, Paged};
use crate::models::required_name;
use crate::services::teams::dedup;

const MAX_NAME: usize = 50;
const MAX_DESCRIPTION: usize = 200;

#[derive(sqlx::FromRow)]
struct LabelRow {
    id: i64,
    name: String,
    color: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
    key_count: i64,
}

const SELECT: &str = "
    SELECT l.id, l.name, l.color, l.description, l.created_at,
           (SELECT COUNT(*) FROM key_labels kl JOIN api_keys k ON k.id = kl.key_id
            WHERE kl.label_id = l.id AND k.revoked_at IS NULL) AS key_count
    FROM labels l";

/// Every label, a page at a time: anyone signed in picks from them.
pub async fn list(pool: &DbPool, list: &ListQuery) -> AppResult<Paged<Label>> {
    let order = list.order_by(
        &[
            ("name", "LOWER(r.name)"),
            ("keys", "r.key_count"),
            ("created_at", "r.created_at"),
        ],
        "name",
        "r.id",
    )?;
    let from = format!(
        "FROM ({SELECT}) AS r
         WHERE ($1 IS NULL OR LOWER(r.name) LIKE $1 ESCAPE '\\'
                OR LOWER(r.description) LIKE $1 ESCAPE '\\')"
    );
    let search = list.search();
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) {from}")))
        .bind(&search)
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as::<_, LabelRow>(sqlx::AssertSqlSafe(format!(
        "SELECT r.* {from} ORDER BY {order} LIMIT $2 OFFSET $3"
    )))
    .bind(&search)
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    Ok(list.paged(rows.into_iter().map(to_label).collect(), total))
}

pub async fn get(pool: &DbPool, id: i64) -> AppResult<Label> {
    sqlx::query_as::<_, LabelRow>(sqlx::AssertSqlSafe(format!("{SELECT} WHERE l.id = $1")))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(to_label)
        .ok_or_else(|| AppError::NotFound(format!("label {id}")))
}

pub async fn create(pool: &DbPool, req: &CreateLabelRequest) -> AppResult<Label> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO labels (name, color, description, created_at) VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(required_name(&req.name, "name", MAX_NAME)?)
    .bind(color(&req.color)?)
    .bind(description(req.description.as_deref())?)
    .bind(Utc::now())
    .fetch_one(pool)
    .await
    .map_err(taken)?;
    get(pool, id).await
}

pub async fn update(pool: &DbPool, id: i64, req: &UpdateLabelRequest) -> AppResult<Label> {
    get(pool, id).await?;
    if let Some(name) = &req.name {
        sqlx::query("UPDATE labels SET name = $1 WHERE id = $2")
            .bind(required_name(name, "name", MAX_NAME)?)
            .bind(id)
            .execute(pool)
            .await
            .map_err(taken)?;
    }
    if let Some(value) = &req.color {
        sqlx::query("UPDATE labels SET color = $1 WHERE id = $2")
            .bind(color(value)?)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(value) = &req.description {
        sqlx::query("UPDATE labels SET description = $1 WHERE id = $2")
            .bind(description(value.as_deref())?)
            .bind(id)
            .execute(pool)
            .await?;
    }
    get(pool, id).await
}

/// Its keys lose it; their spend stays, it just no longer filters by it.
pub async fn delete(pool: &DbPool, id: i64) -> AppResult<()> {
    get(pool, id).await?;
    sqlx::query("DELETE FROM labels WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Each key's labels, by name.
pub async fn by_key(pool: &DbPool) -> AppResult<HashMap<i64, Vec<LabelRef>>> {
    let rows: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT kl.key_id, l.id, l.name, l.color FROM key_labels kl
         JOIN labels l ON l.id = kl.label_id ORDER BY LOWER(l.name)",
    )
    .fetch_all(pool)
    .await?;
    let mut out: HashMap<i64, Vec<LabelRef>> = HashMap::new();
    for (key_id, id, name, color) in rows {
        out.entry(key_id)
            .or_default()
            .push(LabelRef { id, name, color });
    }
    Ok(out)
}

/// Replaces a key's labels, each of which must exist.
pub async fn replace_for_key(
    tx: &mut sqlx::Transaction<'_, Db>,
    key_id: i64,
    label_ids: &[i64],
) -> AppResult<()> {
    sqlx::query("DELETE FROM key_labels WHERE key_id = $1")
        .bind(key_id)
        .execute(&mut **tx)
        .await?;
    for label_id in dedup(label_ids) {
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM labels WHERE id = $1")
            .bind(label_id)
            .fetch_optional(&mut **tx)
            .await?;
        if exists.is_none() {
            return Err(
                AppError::Validation(format!("label {label_id} does not exist"))
                    .with_field("labels", FieldErrorCode::Invalid),
            );
        }
        sqlx::query("INSERT INTO key_labels (key_id, label_id) VALUES ($1, $2)")
            .bind(key_id)
            .bind(label_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn color(value: &str) -> AppResult<&str> {
    COLORS.iter().copied().find(|c| *c == value).ok_or_else(|| {
        AppError::Validation(format!("color must be one of {}", COLORS.join(", ")))
            .with_field("color", FieldErrorCode::Invalid)
    })
}

/// Trimmed, empty as none.
fn description(value: Option<&str>) -> AppResult<Option<String>> {
    match value.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) if d.chars().count() > MAX_DESCRIPTION => Err(AppError::Validation(format!(
            "description is longer than {MAX_DESCRIPTION} characters"
        ))
        .with_field("description", FieldErrorCode::TooLong)),
        other => Ok(other.map(String::from)),
    }
}

fn taken(error: sqlx::Error) -> AppError {
    if is_unique_violation(&error) {
        return AppError::Conflict("a label already has that name".into())
            .with_field("name", FieldErrorCode::AlreadyExists);
    }
    error.into()
}

fn to_label(row: LabelRow) -> Label {
    Label {
        id: row.id,
        name: row.name,
        color: row.color,
        description: row.description,
        created_at: row.created_at,
        key_count: row.key_count,
    }
}
