//! The SQLite store (d0003, d0055): one database file in the data folder, migrations from day one.
//!
//! SQL here stays portable to Postgres (see `migrations/0001_initial.sql`). Known SQLite-only details, kept
//! to this module: the connection options (WAL, `foreign_keys`) and `?` placeholders in query text.

pub mod projects;
pub mod tickets;

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::data_dir::DataDir;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// An open database with every migration applied.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens the database in the data folder, creating the folder and the file on first use.
    pub async fn open_in(data: &DataDir) -> Result<Self, StoreError> {
        DataDir::ensure_dir(data.root()).map_err(StoreError::Folder)?;
        Self::open(&data.database_path()).await
    }

    /// Opens (or creates) the database at `path` and runs any pending migrations. Opening an up-to-date
    /// database changes nothing.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::from_str("sqlite:")?
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Waits for open connections to finish and closes the database.
    pub async fn close(self) {
        self.pool.close().await;
    }
}

/// A new id for any row: a UUIDv7 string, so ids sort roughly by creation time.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Now, as stored in `*_at` columns: milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

#[derive(Debug)]
pub enum StoreError {
    Folder(std::io::Error),
    Database(sqlx::Error),
    Migration(MigrateError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Folder(e) => write!(f, "can't create the data folder: {e}"),
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Migration(e) => write!(f, "database migration failed: {e}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Folder(e) => Some(e),
            Self::Database(e) => Some(e),
            Self::Migration(e) => Some(e),
        }
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e)
    }
}

impl From<MigrateError> for StoreError {
    fn from(e: MigrateError) -> Self {
        Self::Migration(e)
    }
}
