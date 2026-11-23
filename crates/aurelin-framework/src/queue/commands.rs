//! Application-console entry points for failed queue jobs.

use super::Error as QueueError;

/// List terminal failed jobs.
pub async fn failed(database_url: &str, queue: Option<&str>) -> Result<(), Error> {
    let failed = super::failed(database_url, queue).await?;
    if failed.is_empty() {
        println!("No failed jobs found.");
        return Ok(());
    }
    for job in failed {
        println!("{}  {}@{}  failed at {}", job.id, job.connection, job.queue, job.failed_at);
    }
    Ok(())
}

/// Retry terminal failed jobs.
pub async fn retry(database_url: &str, ids: &[String]) -> Result<(), Error> {
    let all_ids;
    let ids = if ids == ["all"] {
        all_ids = super::failed(database_url, None).await?.into_iter().map(|job| job.id).collect::<Vec<_>>();
        &all_ids
    } else {
        ids
    };
    if ids.is_empty() {
        println!("No retryable jobs found.");
        return Ok(());
    }
    for id in ids {
        if !super::retry(database_url, id).await? {
            eprintln!("Unable to find failed job with ID [{id}].");
            continue;
        }
        println!("Failed job [{id}] has been pushed back onto the queue.");
    }
    Ok(())
}

/// Delete terminal failed jobs.
pub async fn forget(database_url: &str, ids: &[String]) -> Result<(), Error> {
    let mut missing = None;
    for id in ids {
        if !super::forget(database_url, id).await? {
            missing.get_or_insert_with(|| id.clone());
            continue;
        }
        println!("Failed job [{id}] deleted successfully.");
    }
    missing.map_or(Ok(()), |id| Err(Error::MissingFailedJob(id)))
}

/// Delete terminal failed jobs matching the retention and queue filters.
pub async fn flush(database_url: &str, hours: Option<u64>, queue: Option<&str>) -> Result<(), Error> {
    let deleted = super::flush(database_url, hours, queue).await?;
    println!("Deleted {deleted} failed job(s).");
    Ok(())
}

/// Failure from an operational queue command.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The queue backend rejected the operation.
    #[error(transparent)]
    Queue(#[from] QueueError),
    /// No terminal failure has the requested identifier.
    #[error("no failed job matches the ID [{0}]")]
    MissingFailedJob(String),
}
