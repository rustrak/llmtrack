use chrono::Utc;

use crate::db::{is_unique_violation, Db, DbPool};
use crate::error::{AppError, AppResult, FieldErrorCode};
use crate::models::list::{ListQuery, Paged};
use crate::models::user::{
    hash_password, is_language_tag, is_time_zone_name, normalize_email, validate_email,
    validate_password, validate_role, UpdateProfileRequest, UpdateUserRequest, User, CURRENCIES,
};

/// The `User` columns, as a literal so queries stay `&'static str`.
macro_rules! columns {
    () => {
        "id, email, password_hash, role, is_active, created_at, last_login, name, language, timezone, currency, currency_rate"
    };
}

pub async fn create<'e>(
    db: impl sqlx::Executor<'e, Database = Db>,
    email: &str,
    password: &str,
    role: &str,
) -> AppResult<User> {
    let email = normalize_email(email);
    validate_email(&email)?;
    validate_password(password, "password")?;
    validate_role(role)?;
    let password_hash = hash_password(password)?;

    sqlx::query_as::<_, User>(concat!(
        "INSERT INTO users (email, password_hash, role, is_active, created_at)
         VALUES ($1, $2, $3, TRUE, $4) RETURNING ",
        columns!()
    ))
    .bind(&email)
    .bind(password_hash)
    .bind(role)
    .bind(Utc::now())
    .fetch_one(db)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            AppError::Conflict(format!("a user with email '{email}' already exists"))
                .with_field("email", FieldErrorCode::AlreadyExists)
        } else {
            e.into()
        }
    })
}

pub async fn get(pool: &DbPool, id: i64) -> AppResult<Option<User>> {
    Ok(
        sqlx::query_as::<_, User>(concat!("SELECT ", columns!(), " FROM users WHERE id = $1"))
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn require(pool: &DbPool, id: i64) -> AppResult<User> {
    get(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {id}")))
}

pub async fn get_by_email(pool: &DbPool, email: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(concat!(
        "SELECT ",
        columns!(),
        " FROM users WHERE email = $1"
    ))
    .bind(normalize_email(email))
    .fetch_optional(pool)
    .await?)
}

/// A user as the users table shows them: with what the keys they created
/// spent, all time, and how many of those keys are active.
#[derive(Debug, serde::Serialize)]
pub struct UserListItem {
    #[serde(flatten)]
    pub user: User,
    pub spend_usd: f64,
    pub key_count: i64,
}

#[derive(sqlx::FromRow)]
struct UserListRow {
    #[sqlx(flatten)]
    user: User,
    spend_nanos: i64,
    key_count: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct UserListQuery {
    /// `admin` or `member`.
    pub role: Option<String>,
    /// `active` or `inactive`.
    pub status: Option<String>,
}

/// The users table, a page at a time.
pub async fn list(
    pool: &DbPool,
    list: &ListQuery,
    filter: &UserListQuery,
) -> AppResult<Paged<UserListItem>> {
    let role = filter.role.as_deref().filter(|r| !r.is_empty());
    if let Some(role) = role {
        validate_role(role)?;
    }
    let active = match filter.status.as_deref() {
        None | Some("") => None,
        Some("active") => Some(true),
        Some("inactive") => Some(false),
        Some(other) => {
            return Err(AppError::Validation(format!(
                "status must be 'active' or 'inactive', not '{other}'"
            ))
            .with_field("status", FieldErrorCode::Invalid))
        }
    };
    let order = list.order_by(
        &[
            ("email", "r.email"),
            ("name", "LOWER(r.name)"),
            ("role", "r.role"),
            ("spend", "r.spend_nanos"),
            ("keys", "r.key_count"),
            ("last_login", "r.last_login"),
            ("created_at", "r.created_at"),
        ],
        "email",
        "r.id",
    )?;
    // Spend is what the keys a user created have spent, all time.
    let from = concat!(
        "FROM (SELECT ",
        columns!(),
        ",
                CAST(COALESCE((SELECT SUM(d.cost_nanos) FROM usage_daily d
                               JOIN api_keys dk ON dk.id = d.key_id
                               WHERE dk.created_by = u.id), 0) AS BIGINT) AS spend_nanos,
                (SELECT COUNT(*) FROM api_keys k
                 WHERE k.created_by = u.id AND k.revoked_at IS NULL) AS key_count
              FROM users u) AS r
         WHERE ($1 IS NULL OR r.role = $1)
           AND ($2 IS NULL OR r.is_active = $2)
           AND ($3 IS NULL OR LOWER(r.email) LIKE $3 ESCAPE '\\'
                OR LOWER(r.name) LIKE $3 ESCAPE '\\')"
    );
    let search = list.search();
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) {from}")))
        .bind(role)
        .bind(active)
        .bind(&search)
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as::<_, UserListRow>(sqlx::AssertSqlSafe(format!(
        "SELECT r.* {from} ORDER BY {order} LIMIT $4 OFFSET $5"
    )))
    .bind(role)
    .bind(active)
    .bind(&search)
    .bind(list.per_page())
    .bind(list.offset())
    .fetch_all(pool)
    .await?;
    let users = rows
        .into_iter()
        .map(|row| UserListItem {
            user: row.user,
            spend_usd: crate::models::money::nanos_to_usd(row.spend_nanos),
            key_count: row.key_count,
        })
        .collect();
    Ok(list.paged(users, total))
}

pub async fn count(pool: &DbPool) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?)
}

pub async fn touch_last_login(pool: &DbPool, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE users SET last_login = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_password(pool: &DbPool, id: i64, password: &str) -> AppResult<()> {
    let hash = hash_password(password)?;
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Applies an admin's edit. `actor_id` cannot demote or disable itself, so the
/// last admin can never lock everyone out by accident.
pub async fn update(
    pool: &DbPool,
    actor_id: i64,
    id: i64,
    req: &UpdateUserRequest,
) -> AppResult<User> {
    let user = require(pool, id).await?;
    if let Some(role) = &req.role {
        validate_role(role)?;
    }
    if let Some(password) = &req.password {
        validate_password(password, "password")?;
    }
    let demotes_self = req.role.as_deref().is_some_and(|r| r != "admin");
    let disables_self = req.is_active == Some(false);
    if id == actor_id && (demotes_self || disables_self) {
        return Err(AppError::Forbidden(
            "you cannot demote or disable your own account".into(),
        ));
    }

    sqlx::query("UPDATE users SET role = $1, is_active = $2 WHERE id = $3")
        .bind(req.role.as_deref().unwrap_or(&user.role))
        .bind(req.is_active.unwrap_or(user.is_active))
        .bind(id)
        .execute(pool)
        .await?;
    if let Some(password) = &req.password {
        set_password(pool, id, password).await?;
    }
    require(pool, id).await
}

pub async fn delete(pool: &DbPool, actor_id: i64, id: i64) -> AppResult<()> {
    if id == actor_id {
        return Err(AppError::Forbidden(
            "you cannot delete your own account".into(),
        ));
    }
    let deleted = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(AppError::NotFound(format!("user {id}")));
    }
    Ok(())
}

/// Creates the first admin from `CREATE_SUPERUSER` (`email:password`), and
/// only into an empty database: setting it again later never resets anyone.
pub async fn ensure_superuser(pool: &DbPool, spec: Option<&str>) -> AppResult<()> {
    let Some(spec) = spec.filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let (email, password) = spec.split_once(':').ok_or_else(|| {
        AppError::Validation("CREATE_SUPERUSER must look like email:password".into())
    })?;
    if count(pool).await? > 0 {
        log::info!("CREATE_SUPERUSER ignored: users already exist");
        return Ok(());
    }
    let user = create(pool, email, password, "admin").await?;
    log::info!("Superuser {} created", user.email);
    Ok(())
}

/// Applies a user's own edit of their profile. Each present field is checked
/// and written; absent ones are left alone.
pub async fn update_profile(pool: &DbPool, id: i64, req: &UpdateProfileRequest) -> AppResult<User> {
    let invalid = |field: &str, what: &str| {
        AppError::Validation(format!("{field} {what}")).with_field(field, FieldErrorCode::Invalid)
    };
    let clean = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(String::from)
    };

    if let Some(name) = &req.name {
        let name = clean(name);
        if name.as_ref().is_some_and(|n| n.chars().count() > 100) {
            return Err(
                AppError::Validation("name is longer than 100 characters".into())
                    .with_field("name", FieldErrorCode::TooLong),
            );
        }
        set_column(pool, id, "name", name).await?;
    }
    if let Some(language) = &req.language {
        let language = clean(language);
        if language.as_deref().is_some_and(|l| !is_language_tag(l)) {
            return Err(invalid("language", "is not a language tag"));
        }
        set_column(pool, id, "language", language).await?;
    }
    if let Some(timezone) = &req.timezone {
        let timezone = clean(timezone);
        if timezone.as_deref().is_some_and(|z| !is_time_zone_name(z)) {
            return Err(invalid("timezone", "is not a time zone name"));
        }
        set_column(pool, id, "timezone", timezone).await?;
    }
    if let Some(currency) = &req.currency {
        let currency = clean(currency);
        if currency
            .as_deref()
            .is_some_and(|c| !CURRENCIES.contains(&c))
        {
            return Err(invalid("currency", "is not a supported currency"));
        }
        set_column(pool, id, "currency", currency).await?;
    }
    if let Some(rate) = req.currency_rate {
        if rate.is_some_and(|r| !r.is_finite() || r <= 0.0) {
            return Err(invalid("currency_rate", "must be above zero"));
        }
        sqlx::query("UPDATE users SET currency_rate = $1 WHERE id = $2")
            .bind(rate)
            .bind(id)
            .execute(pool)
            .await?;
    }
    require(pool, id).await
}

async fn set_column(
    pool: &DbPool,
    id: i64,
    column: &'static str,
    value: Option<String>,
) -> AppResult<()> {
    // `column` is one of the literals above, never input.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE users SET {column} = $1 WHERE id = $2"
    )))
    .bind(value)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// The oldest active admin, whom the master key acts as.
pub async fn first_admin(pool: &DbPool) -> AppResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM users WHERE role = 'admin' AND is_active ORDER BY id LIMIT 1",
    )
    .fetch_optional(pool)
    .await?)
}
