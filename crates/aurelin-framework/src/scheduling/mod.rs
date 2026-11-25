//! Explicit, single-run scheduling of application callbacks.

mod builder;
pub mod commands;
mod report;

use std::{error::Error, future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;

use crate::cache::Cache;

pub use builder::{ConfigurationError, ScheduleBuilder, TaskBuilder};
pub use report::{RunReport, TaskFailure, TaskOutcome, TaskResult};

type BoxError = Box<dyn Error + Send + Sync + 'static>;
type TaskFuture = Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send + 'static>>;
type Callback = dyn Fn() -> TaskFuture + Send + Sync;

struct Task {
    name: String,
    expression: cron::Schedule,
    timezone: Tz,
    callback: Arc<Callback>,
    overlap_seconds: Option<i64>,
    one_server: bool,
}

struct ScheduleInner {
    tasks: Vec<Task>,
    cache: Option<Cache>,
}

/// An immutable schedule. Clones share task definitions and cache handles.
#[derive(Clone)]
pub struct Schedule {
    inner: Arc<ScheduleInner>,
}

impl Schedule {
    /// Begin configuring a schedule.
    pub fn builder() -> ScheduleBuilder {
        ScheduleBuilder::default()
    }

    /// Evaluate one instant, awaiting due tasks in registration order.
    ///
    /// Tasks without single-server election can run on every invocation.
    pub async fn run_due_at(&self, instant: DateTime<Utc>) -> RunReport {
        let minute = instant.with_second(0).and_then(|time| time.with_nanosecond(0)).unwrap_or(instant);
        let mut tasks = Vec::new();
        for task in &self.inner.tasks {
            if !task.expression.includes(minute.with_timezone(&task.timezone)) {
                continue;
            }
            let outcome = self.run_task(task, minute).await;
            tasks.push(TaskResult { name: task.name.clone(), outcome });
        }
        RunReport { tasks }
    }

    /// Evaluate one run using the current UTC clock.
    pub async fn run(&self) -> RunReport {
        self.run_due_at(Utc::now()).await
    }

    async fn run_task(&self, task: &Task, minute: DateTime<Utc>) -> TaskOutcome {
        if task.one_server {
            // The election survives callback completion so later runners lose as well.
            if let Some(cache) = &self.inner.cache {
                let key = format!("{}:occurrence:{}", lock_key(&task.name), minute.timestamp());
                match cache.lock(&key, 3600).acquire().await {
                    Ok(true) => {}
                    Ok(false) => return TaskOutcome::SkippedElection,
                    Err(error) => return TaskOutcome::Failed(TaskFailure::Election(error)),
                }
            }
        }
        let lock = match (task.overlap_seconds, &self.inner.cache) {
            (Some(seconds), Some(cache)) => Some(cache.lock(&lock_key(&task.name), seconds)),
            _ => None,
        };
        if let Some(lock) = &lock {
            match lock.acquire().await {
                Ok(true) => {}
                Ok(false) => return TaskOutcome::SkippedOverlap,
                Err(error) => return TaskOutcome::Failed(TaskFailure::Acquire(error)),
            }
        }
        let result = (task.callback)().await;
        let released = match lock {
            Some(lock) => lock.release().await.map(Some),
            None => Ok(None),
        };
        match (result, released) {
            (Ok(()), Ok(_)) => TaskOutcome::Succeeded,
            (Err(error), Ok(_)) => TaskOutcome::Failed(TaskFailure::Callback(error)),
            (Ok(()), Err(error)) => TaskOutcome::Failed(TaskFailure::Release(error)),
            (Err(callback), Err(release)) => TaskOutcome::Failed(TaskFailure::CallbackAndRelease { callback, release }),
        }
    }
}

/// Derive the stable cache-lock name for a scheduled task.
pub fn lock_key(name: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(name.as_bytes());
    format!("aurelin:schedule:{digest:x}")
}
