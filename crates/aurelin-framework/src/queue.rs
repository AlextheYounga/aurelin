//! Durable background jobs backed by SQLite and Apalis.

use apalis_sqlite::{SqlitePool, SqliteStorage};
use serde::Serialize;
use sqlx::Row;

pub use apalis::prelude::{BoxDynError, Monitor, TaskBuilder, TaskSink, WorkerBuilder, WorkerContext};

pub mod commands;

/// A job that exhausted all of its configured attempts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FailedJob {
    /// Durable task identifier used by retry and forget operations.
    pub id: String,
    /// Queue connection that stored the task.
    pub connection: &'static str,
    /// Apalis queue name associated with the serialized job type.
    pub queue: String,
    /// Serialized application job payload.
    pub payload: Vec<u8>,
    /// Last handler error recorded by the worker.
    pub exception: String,
    /// Unix timestamp when the final attempt completed.
    pub failed_at: i64,
}

/// Connect to an explicitly migrated SQLite queue.
pub async fn sqlite<J>(database_url: &str) -> Result<SqliteStorage<J>, Error> {
    let pool = SqlitePool::connect(database_url).await?;
    ensure_schema(&pool).await?;
    Ok(SqliteStorage::new(&pool))
}

/// List jobs that exhausted all configured attempts, newest first.
pub async fn failed(database_url: &str, queue: Option<&str>) -> Result<Vec<FailedJob>, Error> {
    let pool = SqlitePool::connect(database_url).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query(
        "SELECT id, job_type, job, last_result, done_at \
         FROM Jobs \
         WHERE status = 'Failed' AND attempts >= max_attempts \
           AND (?1 IS NULL OR job_type = ?1) \
         ORDER BY done_at DESC, id DESC",
    )
    .bind(queue)
    .fetch_all(&pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let exception = row.try_get::<Option<String>, _>("last_result")?.unwrap_or_default();
            let exception = serde_json::from_str::<String>(&exception).unwrap_or(exception);
            Ok(FailedJob {
                id: row.try_get("id")?,
                connection: "sqlite",
                queue: row.try_get("job_type")?,
                payload: row.try_get("job")?,
                exception,
                failed_at: row.try_get::<Option<i64>, _>("done_at")?.unwrap_or_default(),
            })
        })
        .collect()
}

/// Make an exhausted job available to a worker again with a fresh attempt count.
pub async fn retry(database_url: &str, id: &str) -> Result<bool, Error> {
    let pool = SqlitePool::connect(database_url).await?;
    ensure_schema(&pool).await?;
    let result = sqlx::query(
        "UPDATE Jobs SET status = 'Pending', attempts = 0, run_at = strftime('%s', 'now'), \
         last_result = NULL, done_at = NULL, lock_at = NULL, lock_by = NULL \
         WHERE id = ?1 AND status = 'Failed' AND attempts >= max_attempts",
    )
    .bind(id)
    .execute(&pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Delete an exhausted job permanently.
pub async fn forget(database_url: &str, id: &str) -> Result<bool, Error> {
    let pool = SqlitePool::connect(database_url).await?;
    ensure_schema(&pool).await?;
    let result = sqlx::query("DELETE FROM Jobs WHERE id = ?1 AND status = 'Failed' AND attempts >= max_attempts")
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Delete exhausted jobs, optionally retaining recent failures or filtering by queue.
pub async fn flush(database_url: &str, hours: Option<u64>, queue: Option<&str>) -> Result<u64, Error> {
    let pool = SqlitePool::connect(database_url).await?;
    ensure_schema(&pool).await?;
    let retained_seconds = hours
        .map(|hours| hours.saturating_mul(60).saturating_mul(60))
        .map(i64::try_from)
        .transpose()
        .map_err(|_| Error::InvalidRetention)?;
    let result = sqlx::query(
        "DELETE FROM Jobs \
         WHERE status = 'Failed' AND attempts >= max_attempts \
           AND (?1 IS NULL OR done_at <= strftime('%s', 'now') - ?1) \
           AND (?2 IS NULL OR job_type = ?2)",
    )
    .bind(retained_seconds)
    .bind(queue)
    .execute(&pool)
    .await?;
    Ok(result.rows_affected())
}

async fn ensure_schema(pool: &SqlitePool) -> Result<(), Error> {
    let tables = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' AND lower(name) IN ('jobs', 'workers')",
    )
    .fetch_one(pool)
    .await?;
    if tables != 2 {
        return Err(Error::PendingMigrations("queue migrations are pending; run `cargo aurelin migrate`"));
    }
    Ok(())
}

/// Failures encountered while opening or preparing a SQLite queue.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The SQLite connection could not be opened.
    #[error("connect to the queue database: {0}")]
    Connect(#[from] apalis_sqlite::SqlxError),
    /// The generated application's queue migration has not been applied.
    #[error("{0}")]
    PendingMigrations(&'static str),
    /// The requested failed-job retention cannot be represented by SQLite.
    #[error("failed-job retention is too large")]
    InvalidRetention,
}
