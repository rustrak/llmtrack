//! Pool, migrations and the few SQL fragments the two backends spell
//! differently. Everything else is written once, in SQL both accept.
//!
//! Timestamps are always bound from Rust, never defaulted by the database:
//! SQLite's `datetime('now')` and sqlx's encoding of `DateTime<Utc>` are
//! different strings, and SQLite compares timestamps as strings.

#[cfg(all(feature = "postgres", feature = "sqlite"))]
compile_error!("Features \"postgres\" and \"sqlite\" are mutually exclusive. Enable only one.");

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("Either feature \"postgres\" or \"sqlite\" must be enabled.");

use std::time::Duration;

#[cfg(feature = "postgres")]
pub type Db = sqlx::Postgres;
#[cfg(feature = "sqlite")]
pub type Db = sqlx::Sqlite;

pub type DbPool = sqlx::Pool<Db>;

/// The UTC calendar day of `created_at`, as `YYYY-MM-DD` text.
#[cfg(feature = "postgres")]
pub const DAY_OF_CREATED_AT: &str = "to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD')";
/// The UTC calendar day of `created_at`. sqlx stores RFC 3339, so the first
/// ten characters are the date.
#[cfg(feature = "sqlite")]
pub const DAY_OF_CREATED_AT: &str = "substr(created_at, 1, 10)";

/// The rows of the JSON array `expr` (text), as a table with one column,
/// `value`: what a list filter (`models::list::List::json`) and the request
/// tags are read through.
pub fn each(expr: &str) -> String {
    if cfg!(feature = "postgres") {
        format!("jsonb_array_elements_text(CAST({expr} AS JSONB))")
    } else {
        format!("json_each({expr})")
    }
}

/// The request ids whose stored body holds every word of `$12`, a
/// [`text_query`]: one full-text index lookup, whatever the size of the logs.
#[cfg(feature = "postgres")]
pub const BODIES_MATCHING_12: &str =
    "SELECT b.request_id FROM request_bodies b WHERE b.search @@ plainto_tsquery('simple', $12)";
#[cfg(feature = "sqlite")]
pub const BODIES_MATCHING_12: &str = "SELECT b.request_id FROM request_bodies b WHERE b.rowid IN
     (SELECT rowid FROM request_bodies_fts WHERE request_bodies_fts MATCH $12)";

/// Free text as the full-text query of [`BODIES_MATCHING_12`]: every word
/// must appear. `None` when there are no words. Each word is quoted for
/// SQLite, so nothing a user types is read as FTS5 syntax.
pub fn text_query(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    if cfg!(feature = "postgres") {
        return Some(words.join(" "));
    }
    Some(
        words
            .iter()
            .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub async fn create_pool(url: &str, max_connections: u32) -> Result<DbPool, sqlx::Error> {
    #[cfg(feature = "postgres")]
    {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(5))
            .connect(url)
            .await
    }

    #[cfg(feature = "sqlite")]
    {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
        use std::str::FromStr;

        let in_memory = url.contains(":memory:") || url.contains("mode=memory");
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);
        sqlx::sqlite::SqlitePoolOptions::new()
            // Every connection to `:memory:` is its own empty database.
            .max_connections(if in_memory { 1 } else { max_connections })
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
    }
}

pub async fn run_migrations(pool: &DbPool) -> Result<(), sqlx::migrate::MigrateError> {
    #[cfg(feature = "postgres")]
    sqlx::migrate!("./migrations/postgres").run(pool).await?;
    #[cfg(feature = "sqlite")]
    sqlx::migrate!("./migrations/sqlite").run(pool).await?;
    Ok(())
}

/// Opens a write transaction. SQLite's `BEGIN IMMEDIATE` takes the write lock
/// up front, so two writers queue on `busy_timeout` instead of one failing
/// with SQLITE_BUSY halfway through.
pub async fn begin_write(pool: &DbPool) -> Result<sqlx::Transaction<'_, Db>, sqlx::Error> {
    #[cfg(feature = "sqlite")]
    {
        pool.begin_with("BEGIN IMMEDIATE").await
    }
    #[cfg(feature = "postgres")]
    {
        pool.begin().await
    }
}

pub async fn health_check(pool: &DbPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

/// True when `error` is a unique-constraint violation, which callers turn
/// into a 409 naming the field.
pub fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.is_unique_violation())
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    #[test]
    fn a_text_query_quotes_every_word() {
        assert_eq!(super::text_query("  "), None);
        assert_eq!(
            super::text_query("refund  \"NEAR\" OR"),
            Some("\"refund\" \"\"\"NEAR\"\"\" \"OR\"".into())
        );
    }
}
