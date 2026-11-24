use std::{collections::HashSet, error::Error, future::Future, str::FromStr, sync::Arc, time::Duration};

use chrono_tz::Tz;

use super::{BoxError, Callback, Schedule, ScheduleInner, Task, TaskFuture};
use crate::cache::Cache;

/// Invalid schedule configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    /// Task names must be nonempty and unique.
    #[error("task name must be nonempty and unique: {0:?}")]
    InvalidName(String),
    /// Only the documented cron subset is accepted.
    #[error("invalid or unsupported cron expression {expression:?}: {reason}")]
    InvalidCron { expression: String, reason: String },
    /// The supplied timezone is not an IANA timezone.
    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),
    /// Protected tasks require a cache and a positive whole-second TTL.
    #[error("overlap prevention for {0:?} requires a cache and a positive whole-second lifetime")]
    InvalidOverlap(String),
}

/// Configures one application-owned callback.
pub struct TaskBuilder {
    name: String,
    expression: String,
    timezone: String,
    callback: Arc<Callback>,
    overlap: Option<Duration>,
}

impl TaskBuilder {
    /// Create a task with a stable name, five-field cron expression, and async callback.
    pub fn new<F, Fut, E>(name: impl Into<String>, expression: impl Into<String>, callback: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
        E: Error + Send + Sync + 'static,
    {
        let callback = Arc::new(move || {
            let future = callback();
            Box::pin(async move { future.await.map_err(|error| Box::new(error) as BoxError) }) as TaskFuture
        });
        Self { name: name.into(), expression: expression.into(), timezone: "UTC".into(), callback, overlap: None }
    }

    /// Evaluate this task in an IANA timezone instead of UTC.
    pub fn timezone(mut self, timezone: impl Into<String>) -> Self {
        self.timezone = timezone.into();
        self
    }

    /// Skip overlapping executions, using Laravel's default 24-hour lock lifetime.
    pub fn without_overlapping(mut self) -> Self {
        self.overlap = Some(Duration::from_secs(86_400));
        self
    }

    /// Skip overlapping executions, using a positive whole-second lock lifetime.
    pub fn overlap_lifetime(mut self, lifetime: Duration) -> Self {
        self.overlap = Some(lifetime);
        self
    }
}

/// Builds a validated, immutable schedule.
#[derive(Default)]
pub struct ScheduleBuilder {
    tasks: Vec<TaskBuilder>,
    cache: Option<Cache>,
}

impl ScheduleBuilder {
    /// Select the shared cache used for protected tasks.
    pub fn cache(&mut self, cache: Cache) -> &mut Self {
        self.cache = Some(cache);
        self
    }

    /// Register a callback after previously registered tasks.
    pub fn task(&mut self, task: TaskBuilder) -> &mut Self {
        self.tasks.push(task);
        self
    }

    /// Validate configuration and finish the schedule.
    pub fn build(self) -> Result<Schedule, ConfigurationError> {
        let mut names = HashSet::new();
        let mut tasks = Vec::with_capacity(self.tasks.len());
        for task in self.tasks {
            if task.name.trim().is_empty() || !names.insert(task.name.clone()) {
                return Err(ConfigurationError::InvalidName(task.name));
            }
            let timezone =
                Tz::from_str(&task.timezone).map_err(|_| ConfigurationError::InvalidTimezone(task.timezone))?;
            let expression = parse_expression(&task.expression)?;
            let overlap_seconds = match task.overlap {
                Some(duration) if self.cache.is_some() && duration.subsec_nanos() == 0 => {
                    let seconds = i64::try_from(duration.as_secs())
                        .ok()
                        .filter(|value| *value > 0)
                        .ok_or_else(|| ConfigurationError::InvalidOverlap(task.name.clone()))?;
                    Some(seconds)
                }
                Some(_) => return Err(ConfigurationError::InvalidOverlap(task.name)),
                None => None,
            };
            tasks.push(Task { name: task.name, expression, timezone, callback: task.callback, overlap_seconds });
        }
        Ok(Schedule { inner: Arc::new(ScheduleInner { tasks, cache: self.cache }) })
    }
}

fn parse_expression(expression: &str) -> Result<cron::Schedule, ConfigurationError> {
    let fields: Vec<_> = expression.split_whitespace().collect();
    let invalid =
        |reason: &str| ConfigurationError::InvalidCron { expression: expression.into(), reason: reason.into() };
    if fields.len() != 5 {
        return Err(invalid("expected five cron fields"));
    }
    // cron's weekday ordinals differ from Laravel's (Sunday is 1, not 0).
    // Both restricted day fields also have different matching semantics.
    if fields[4].bytes().any(|byte| byte.is_ascii_digit()) {
        return Err(invalid("numeric weekdays are unsupported; use weekday names"));
    }
    if fields[2] != "*" && fields[4] != "*" {
        return Err(invalid("restrict either day of month or day of week, not both"));
    }
    cron::Schedule::from_str(&format!("0 {expression}")).map_err(|error| invalid(&error.to_string()))
}
