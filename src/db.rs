//! SQLite pool + embedded migrations.
//!
//! Queries are written with sqlx's runtime API (`query`, `query_as` + `FromRow`),
//! not the compile-time `query!` macros, so the crate builds without a database.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::migrate::MigrateError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

/// Opens (creating if missing) the database at `url`, e.g. `sqlite://cliuno.db`.
pub async fn connect(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
}

/// A private in-memory database for tests. It lives as long as its single
/// connection, so the pool never lets that connection go.
pub async fn connect_in_memory() -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
}

/// Applies every pending file in `./migrations` (`<timestamp>_<name>.sql`).
pub async fn migrate(pool: &SqlitePool) -> Result<(), MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
