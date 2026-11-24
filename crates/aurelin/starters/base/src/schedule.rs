use aurelin_framework::scheduling::{ConfigurationError, Schedule};

/// Register this application's scheduled tasks and their dependencies here.
pub fn schedule() -> Result<Schedule, ConfigurationError> {
    Schedule::builder().build()
}
