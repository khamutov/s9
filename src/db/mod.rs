#![allow(dead_code)]

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

/// Initialize the SQLite connection pool with recommended pragmas.
///
/// Configures WAL journal mode, foreign keys, busy timeout, and synchronous mode
/// per the database design document.
pub async fn init_pool(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let options = SqliteConnectOptions::from_str(&url)?;

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;

    // Apply pragmas for performance and correctness.
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;

    Ok(pool)
}

/// Creates an in-memory SQLite pool with the same pragmas as [`init_pool`].
///
/// Intended for integration tests — each call produces an isolated database.
pub async fn init_memory_pool() -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;

    Ok(pool)
}

/// Run pending sqlx migrations from the `migrations/` directory.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
