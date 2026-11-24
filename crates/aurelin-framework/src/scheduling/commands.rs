//! Process-facing scheduler command.

use super::{Schedule, TaskOutcome};

/// Run due tasks, report failures to stderr, and signal a failing process status.
pub async fn run(schedule: &Schedule) -> Result<(), CommandError> {
    let report = schedule.run().await;
    for task in report.tasks() {
        if let TaskOutcome::Failed(error) = task.outcome() {
            eprintln!("scheduled task [{}] failed: {error}", task.name());
        }
    }
    if report.has_failures() { Err(CommandError) } else { Ok(()) }
}

/// At least one due scheduled task failed.
#[derive(Debug, thiserror::Error)]
#[error("one or more scheduled tasks failed")]
pub struct CommandError;
