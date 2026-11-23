use std::{
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use apalis_sqlite::{SqlitePool, SqliteStorage};
use aurelin_framework::queue::{self, BoxDynError, TaskBuilder, TaskSink, WorkerBuilder, WorkerContext};
use tokio::time::sleep;

async fn migrated_database() -> Result<(tempfile::NamedTempFile, String), Box<dyn Error>> {
    let database = tempfile::NamedTempFile::new()?;
    let database_url = format!("sqlite://{}?mode=rwc", database.path().display());
    let pool = SqlitePool::connect(&database_url).await?;
    SqliteStorage::setup(&pool).await?;
    pool.close().await;
    Ok((database, database_url))
}

#[tokio::test]
async fn sqlite_queue_dispatches_a_job_to_a_worker() -> Result<(), Box<dyn Error>> {
    let (_database, database_url) = migrated_database().await?;
    let mut queue = queue::sqlite::<usize>(&database_url).await?;
    queue.push(42).await?;

    let executions = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&executions);
    let worker = WorkerBuilder::new("queue-test").backend(queue).build(move |job: usize, worker: WorkerContext| {
        let observed = Arc::clone(&observed);
        async move {
            observed.store(job, Ordering::SeqCst);
            worker.stop()?;
            Ok::<(), BoxDynError>(())
        }
    });

    worker.run().await?;
    assert_eq!(executions.load(Ordering::SeqCst), 42);
    Ok(())
}

#[tokio::test]
async fn failed_jobs_are_retried_until_their_configured_attempt_limit() -> Result<(), Box<dyn Error>> {
    let (_database, database_url) = migrated_database().await?;
    let mut queue = queue::sqlite::<usize>(&database_url).await?;
    queue.push_task(TaskBuilder::new(42).max_attempts(2).build()).await?;

    let executions = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&executions);
    WorkerBuilder::new("retry-test")
        .backend(queue)
        .build(move |_job: usize, worker: WorkerContext| {
            let observed = Arc::clone(&observed);
            async move {
                let attempt = observed.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt == 1 {
                    return Err::<(), BoxDynError>(io::Error::other("transient failure").into());
                }
                worker.stop()?;
                Ok::<(), BoxDynError>(())
            }
        })
        .run()
        .await?;

    assert_eq!(executions.load(Ordering::SeqCst), 2);
    assert!(queue::failed(&database_url, None).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn an_exhausted_job_is_recorded_without_preventing_other_work() -> Result<(), Box<dyn Error>> {
    let (_database, database_url) = migrated_database().await?;
    let mut queue = queue::sqlite::<usize>(&database_url).await?;
    queue.push_task(TaskBuilder::new(1).max_attempts(1).build()).await?;
    queue.push_task(TaskBuilder::new(2).max_attempts(1).build()).await?;

    let completed = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&completed);
    WorkerBuilder::new("exhaustion-test")
        .backend(queue)
        .build(move |job: usize, _worker: WorkerContext| {
            let observed = Arc::clone(&observed);
            async move {
                if job == 1 {
                    return Err::<(), BoxDynError>(io::Error::other("permanent failure").into());
                }
                observed.store(job, Ordering::SeqCst);
                Ok::<(), BoxDynError>(())
            }
        })
        .run_with_ctx(|worker| async move {
            sleep(Duration::from_millis(50)).await;
            worker.stop()?;
            Ok(())
        })
        .await?;

    assert_eq!(completed.load(Ordering::SeqCst), 2);
    let failed = queue::failed(&database_url, None).await?;
    assert_eq!(failed.len(), 1);
    assert_eq!(serde_json::from_slice::<usize>(&failed[0].payload)?, 1);
    assert!(failed[0].exception.contains("permanent failure"));
    Ok(())
}

#[tokio::test]
async fn sqlite_queue_requires_explicit_application_migrations() -> Result<(), Box<dyn Error>> {
    let database = tempfile::NamedTempFile::new()?;
    let database_url = format!("sqlite://{}?mode=rwc", database.path().display());

    let error = queue::sqlite::<usize>(&database_url).await.expect_err("schema should be required");

    assert_eq!(error.to_string(), "queue migrations are pending; run `cargo aurelin migrate`");
    Ok(())
}

#[tokio::test]
async fn exhausted_jobs_can_be_listed_retried_and_forgotten() -> Result<(), Box<dyn Error>> {
    let (_database, database_url) = migrated_database().await?;
    let pool = SqlitePool::connect(&database_url).await?;
    sqlx::query(
        "INSERT INTO Jobs \
         (job, id, job_type, status, attempts, max_attempts, run_at, last_result, done_at, priority) \
         VALUES (?1, ?2, ?3, 'Failed', 2, 2, 0, ?4, 100, 0)",
    )
    .bind(serde_json::to_vec(&42usize)?)
    .bind("01TESTFAILEDJOB000000000000")
    .bind("example")
    .bind(serde_json::to_string("handler failed")?)
    .execute(&pool)
    .await?;

    let failed = queue::failed(&database_url, None).await?;
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].id, "01TESTFAILEDJOB000000000000");
    assert_eq!(failed[0].exception, "handler failed");

    queue::commands::retry(&database_url, &["all".to_owned()]).await?;
    assert!(queue::failed(&database_url, None).await?.is_empty());

    sqlx::query("UPDATE Jobs SET status = 'Failed', attempts = max_attempts, done_at = 101 WHERE id = ?1")
        .bind(&failed[0].id)
        .execute(&pool)
        .await?;
    assert!(queue::forget(&database_url, &failed[0].id).await?);
    assert!(!queue::forget(&database_url, &failed[0].id).await?);

    let missing = ["missing-job".to_owned()];
    queue::commands::retry(&database_url, &missing).await?;
    let error =
        queue::commands::forget(&database_url, &missing).await.expect_err("unknown failed jobs should be rejected");
    assert_eq!(error.to_string(), "no failed job matches the ID [missing-job]");
    Ok(())
}

#[tokio::test]
async fn failed_jobs_can_be_flushed_by_age_and_queue_without_deleting_other_jobs() -> Result<(), Box<dyn Error>> {
    let (_database, database_url) = migrated_database().await?;
    let pool = SqlitePool::connect(&database_url).await?;
    for (id, queue_name, failed_at) in [
        ("01OLDDEFAULT00000000000000", "default", 1_i64),
        ("01OLDMAIL0000000000000000", "mail", 1_i64),
        ("01RECENTDEFAULT000000000000", "default", i64::MAX),
    ] {
        sqlx::query(
            "INSERT INTO Jobs \
             (job, id, job_type, status, attempts, max_attempts, run_at, last_result, done_at, priority) \
             VALUES (?1, ?2, ?3, 'Failed', 1, 1, 0, 'failed', ?4, 0)",
        )
        .bind(serde_json::to_vec(&id)?)
        .bind(id)
        .bind(queue_name)
        .bind(failed_at)
        .execute(&pool)
        .await?;
    }

    assert_eq!(queue::flush(&database_url, Some(1), Some("default")).await?, 1);
    let remaining = queue::failed(&database_url, None).await?;
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().any(|job| job.queue == "mail"));
    assert!(remaining.iter().any(|job| job.id == "01RECENTDEFAULT000000000000"));
    Ok(())
}
