use std::time::Duration;

use tokio::time::{Instant, sleep_until};

/// Ensures that a failed operation takes at least a configured duration.
#[derive(Clone, Copy, Debug)]
pub struct Timebox {
    deadline: Instant,
}

impl Timebox {
    #[must_use]
    pub fn start(duration: Duration) -> Self {
        Self { deadline: Instant::now() + duration }
    }

    pub async fn wait(self) {
        sleep_until(self.deadline).await;
    }
}
