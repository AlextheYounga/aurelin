use std::{
    convert::Infallible,
    env,
    error::Error,
    io, process,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use aurelin_framework::{
    cache::{Cache, Error as CacheError, LocalStore, RedisStore},
    scheduling::{Schedule, TaskBuilder, TaskOutcome, lock_key},
};
use chrono::{TimeZone, Utc};
use tokio::{
    sync::Notify,
    time::{sleep, timeout},
};

type TestResult = Result<(), Box<dyn Error>>;

fn instant() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 12, 30, 35).single().unwrap_or_default()
}

fn no_op(name: &str, cron: &str) -> TaskBuilder {
    TaskBuilder::new(name, cron, || async { Ok::<_, Infallible>(()) })
}

fn cache() -> Cache {
    Cache::new(LocalStore::new(1000), "test:")
}

#[test]
fn invalid_schedules_are_rejected_before_execution() {
    for expression in ["invalid", "* * * * 0", "* * 1 * MON", "* 77 * * *"] {
        let mut builder = Schedule::builder();
        builder.task(no_op("bad", expression));
        assert!(builder.build().is_err(), "{expression}");
    }
    let mut builder = Schedule::builder();
    builder.task(no_op("task", "* * * * *").timezone("Not/AZone"));
    assert!(builder.build().is_err());
    let mut builder = Schedule::builder();
    builder.task(no_op("", "* * * * *"));
    assert!(builder.build().is_err());
    let mut builder = Schedule::builder();
    builder.task(no_op("same", "* * * * *")).task(no_op("same", "* * * * *"));
    assert!(builder.build().is_err());
    let mut builder = Schedule::builder();
    builder.task(no_op("protected", "* * * * *").without_overlapping());
    assert!(builder.build().is_err());
    let mut builder = Schedule::builder();
    builder.task(no_op("elected", "* * * * *").on_one_server());
    assert!(builder.build().is_err());
}

#[tokio::test]
async fn independent_schedulers_elect_one_runner_per_occurrence_even_after_failure() -> TestResult {
    let store = LocalStore::new(1000);
    let count = Arc::new(AtomicUsize::new(0));
    let make_schedule = || -> Result<Schedule, Box<dyn Error>> {
        let mut builder = Schedule::builder();
        let count = Arc::clone(&count);
        builder.cache(Cache::new(store.clone(), "election:"));
        builder.task(
            TaskBuilder::new("elected", "* * * * *", move || {
                let count = Arc::clone(&count);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(io::Error::other("callback failed"))
                }
            })
            .on_one_server(),
        );
        Ok(builder.build()?)
    };
    let first = make_schedule()?;
    let second = make_schedule()?;
    let (a, b) = tokio::join!(first.run_due_at(instant()), second.run_due_at(instant()));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(matches!(
        (a.tasks()[0].outcome(), b.tasks()[0].outcome()),
        (TaskOutcome::Failed(_), TaskOutcome::SkippedElection) | (TaskOutcome::SkippedElection, TaskOutcome::Failed(_))
    ));
    assert!(matches!(second.run_due_at(instant()).await.tasks()[0].outcome(), TaskOutcome::SkippedElection));
    assert!(matches!(
        second.run_due_at(instant() + chrono::Duration::minutes(1)).await.tasks()[0].outcome(),
        TaskOutcome::Failed(_)
    ));
    assert_eq!(count.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn due_tasks_run_in_order_and_failures_do_not_stop_later_tasks() -> TestResult {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut builder = Schedule::builder();
    for name in ["first", "fails", "last"] {
        let seen = Arc::clone(&seen);
        builder.task(TaskBuilder::new(name, "30 12 * * *", move || {
            let seen = Arc::clone(&seen);
            async move {
                seen.lock().map_err(|_| io::Error::other("poisoned"))?.push(name);
                if name == "fails" { Err(io::Error::other("task error")) } else { Ok(()) }
            }
        }));
    }
    builder.task(no_op("not due", "31 12 * * *"));
    let schedule = builder.build()?;
    let report = schedule.clone().run_due_at(instant()).await;
    assert_eq!(*seen.lock().map_err(|_| io::Error::other("poisoned"))?, vec!["first", "fails", "last"]);
    assert_eq!(report.tasks().iter().map(|task| task.name()).collect::<Vec<_>>(), vec!["first", "fails", "last"]);
    assert!(report.has_failures());
    assert!(matches!(report.tasks()[1].outcome(), TaskOutcome::Failed(_)));
    let empty = schedule.run_due_at(instant() + chrono::Duration::minutes(2)).await;
    assert!(empty.tasks().is_empty());
    assert!(!empty.has_failures());
    Ok(())
}

#[tokio::test]
async fn configured_timezone_evaluates_the_same_instant_in_local_time() -> TestResult {
    let mut builder = Schedule::builder();
    builder.task(no_op("new-york", "30 7 * * TUE").timezone("America/New_York"));
    let report = builder.build()?.run_due_at(instant()).await;
    assert_eq!(report.tasks().len(), 1);
    Ok(())
}

#[tokio::test]
async fn daylight_saving_transitions_follow_local_clock_time() -> TestResult {
    let mut builder = Schedule::builder();
    builder.task(no_op("repeated", "30 1 * * *").timezone("America/New_York"));
    builder.task(no_op("missing", "30 2 * * *").timezone("America/New_York"));
    let schedule = builder.build()?;
    for hour in [5, 6] {
        let time = Utc.with_ymd_and_hms(2026, 11, 1, hour, 30, 0).single().ok_or("invalid time")?;
        let report = schedule.run_due_at(time).await;
        assert_eq!(report.tasks().iter().map(|task| task.name()).collect::<Vec<_>>(), vec!["repeated"]);
    }
    for hour in [6, 7] {
        let time = Utc.with_ymd_and_hms(2026, 3, 8, hour, 30, 0).single().ok_or("invalid time")?;
        let report = schedule.run_due_at(time).await;
        assert!(!report.tasks().iter().any(|task| task.name() == "missing"));
    }
    Ok(())
}

#[tokio::test]
async fn overlapping_runs_skip_the_same_task_without_blocking_unprotected_tasks() -> TestResult {
    let started = Arc::new(Notify::new());
    let finish = Arc::new(Notify::new());
    let count = Arc::new(AtomicUsize::new(0));
    let mut builder = Schedule::builder();
    builder.cache(cache());
    let (s, f, c) = (Arc::clone(&started), Arc::clone(&finish), Arc::clone(&count));
    builder.task(
        TaskBuilder::new("protected", "* * * * *", move || {
            let (s, f, c) = (Arc::clone(&s), Arc::clone(&f), Arc::clone(&c));
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                s.notify_one();
                f.notified().await;
                Ok::<_, Infallible>(())
            }
        })
        .without_overlapping(),
    );
    builder.task(no_op("unprotected", "* * * * *"));
    let schedule = builder.build()?;
    let first = {
        let schedule = schedule.clone();
        tokio::spawn(async move { schedule.run_due_at(instant()).await })
    };
    started.notified().await;
    let second = schedule.run_due_at(instant()).await;
    assert!(matches!(second.tasks()[0].outcome(), TaskOutcome::SkippedOverlap));
    assert!(matches!(second.tasks()[1].outcome(), TaskOutcome::Succeeded));
    finish.notify_one();
    let first = first.await?;
    assert!(!first.has_failures());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn protected_tasks_release_locks_after_success_and_callback_errors() -> TestResult {
    let cache = cache();
    let mut builder = Schedule::builder();
    builder.cache(cache.clone());
    builder.task(no_op("success", "* * * * *").without_overlapping());
    builder.task(
        TaskBuilder::new("failure", "* * * * *", || async { Err::<(), _>(io::Error::other("failure")) })
            .without_overlapping(),
    );
    let schedule = builder.build()?;
    let held = cache.lock(&lock_key("success"), 60);
    assert!(held.acquire().await?);
    let report = schedule.run_due_at(instant()).await;
    assert!(matches!(report.tasks()[0].outcome(), TaskOutcome::SkippedOverlap));
    assert!(matches!(report.tasks()[1].outcome(), TaskOutcome::Failed(_)));
    assert!(cache.lock(&lock_key("failure"), 60).acquire().await?);
    assert!(held.release().await?);
    let result = schedule.run_due_at(instant()).await;
    assert!(matches!(result.tasks()[0].outcome(), TaskOutcome::Succeeded));
    assert!(cache.lock(&lock_key("success"), 60).acquire().await?);
    assert_ne!(lock_key("success"), lock_key("failure"));
    Ok(())
}

#[tokio::test]
async fn expired_lock_allows_a_later_execution_without_owner_confusion() -> TestResult {
    let cache = cache();
    let old = cache.lock(&lock_key("expiring"), 1);
    assert!(old.acquire().await?);
    let mut builder = Schedule::builder();
    builder.cache(cache.clone()).task(no_op("expiring", "* * * * *").overlap_lifetime(Duration::from_secs(30)));
    let schedule = builder.build()?;
    assert!(matches!(schedule.run_due_at(instant()).await.tasks()[0].outcome(), TaskOutcome::SkippedOverlap));
    timeout(Duration::from_secs(5), async {
        loop {
            if !old.is_locked().await? {
                return Ok::<_, CacheError>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    let replacement = cache.lock(&lock_key("expiring"), 30);
    assert!(replacement.acquire().await?);
    assert!(!old.release().await?);
    assert!(replacement.is_owned().await?);
    assert!(replacement.release().await?);
    assert!(matches!(schedule.run_due_at(instant()).await.tasks()[0].outcome(), TaskOutcome::Succeeded));
    Ok(())
}

#[tokio::test]
async fn redis_cache_blocks_overlapping_scheduled_callbacks_when_configured() -> TestResult {
    let Some(url) = env::var_os("AURELIN_TEST_REDIS_URL") else {
        return Ok(());
    };
    let store = RedisStore::connect(&url.to_string_lossy()).await?;
    let cache = Cache::new(store, format!("scheduling-test-{}:", process::id()));
    let held = cache.lock(&lock_key("redis-protected"), 30);
    assert!(held.acquire().await?);
    let mut builder = Schedule::builder();
    builder.cache(cache.clone()).task(no_op("redis-protected", "* * * * *").without_overlapping());
    let schedule = builder.build()?;
    assert!(matches!(schedule.run_due_at(instant()).await.tasks()[0].outcome(), TaskOutcome::SkippedOverlap));
    assert!(held.release().await?);
    assert!(matches!(schedule.run_due_at(instant()).await.tasks()[0].outcome(), TaskOutcome::Succeeded));
    Ok(())
}
