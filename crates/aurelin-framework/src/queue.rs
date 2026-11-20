//! Durable background jobs backed by SQLite and Apalis.

use apalis_sqlite::{SqlitePool, SqliteStorage};

pub use apalis::prelude::{BoxDynError, TaskSink, WorkerBuilder, WorkerContext};

/// Connect to a SQLite queue and create its storage schema when necessary.
pub async fn sqlite<J>(database_url: &str) -> Result<SqliteStorage<J>, Error> {
    let pool = SqlitePool::connect(database_url).await?;
    SqliteStorage::setup(&pool).await?;
    Ok(SqliteStorage::new(&pool))
}

/// Failures encountered while opening or preparing a SQLite queue.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The SQLite connection could not be opened.
    #[error("connect to the queue database: {0}")]
    Connect(#[from] apalis_sqlite::SqlxError),
    /// The queue schema could not be prepared.
    #[error("prepare the queue database: {0}")]
    Setup(#[from] apalis_sqlite::Error),
}
