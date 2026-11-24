use super::BoxError;
use crate::cache::Error as CacheError;

/// Outcomes of all tasks due during one scheduler invocation, in registration order.
#[derive(Debug)]
pub struct RunReport {
    pub(crate) tasks: Vec<TaskResult>,
}

impl RunReport {
    /// The due tasks and their outcomes.
    pub fn tasks(&self) -> &[TaskResult] {
        &self.tasks
    }

    /// Whether any due task failed.
    pub fn has_failures(&self) -> bool {
        self.tasks.iter().any(|task| matches!(task.outcome, TaskOutcome::Failed(_)))
    }
}

/// The result of one due task.
#[derive(Debug)]
pub struct TaskResult {
    pub(crate) name: String,
    pub(crate) outcome: TaskOutcome,
}

impl TaskResult {
    /// Application-owned stable task name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// How this task completed.
    pub fn outcome(&self) -> &TaskOutcome {
        &self.outcome
    }
}

/// An observable task outcome.
#[derive(Debug)]
pub enum TaskOutcome {
    /// The callback completed successfully.
    Succeeded,
    /// Another execution held the overlap lock.
    SkippedOverlap,
    /// The callback or its lock operation failed.
    Failed(TaskFailure),
}

/// Failure at a task or cache-lock boundary.
#[derive(Debug, thiserror::Error)]
pub enum TaskFailure {
    /// The lock could not be acquired because the cache failed.
    #[error("could not acquire overlap lock: {0}")]
    Acquire(#[source] CacheError),
    /// The application callback returned an error.
    #[error("callback failed: {0}")]
    Callback(#[source] BoxError),
    /// The cache failed while releasing the lock.
    #[error("could not release overlap lock: {0}")]
    Release(#[source] CacheError),
    /// The callback and lock release both failed.
    #[error("callback failed: {callback}; could not release overlap lock: {release}")]
    CallbackAndRelease {
        /// Original callback error.
        callback: BoxError,
        /// Error encountered during cleanup.
        release: CacheError,
    },
}
