use std::time::Duration;

use aurelin_framework::auth::{FixedWindowRateLimiter, RateLimitOutcome};

pub const TWO_FACTOR_ATTEMPTS_PER_MINUTE: u32 = 5;

#[derive(Clone, Debug)]
pub struct TwoFactorRateLimiter {
    limiter: FixedWindowRateLimiter,
}

impl Default for TwoFactorRateLimiter {
    fn default() -> Self {
        Self { limiter: FixedWindowRateLimiter::new(TWO_FACTOR_ATTEMPTS_PER_MINUTE, Duration::from_secs(60)) }
    }
}

impl TwoFactorRateLimiter {
    pub async fn attempt(&self, user_id: i64) -> RateLimitOutcome {
        self.limiter.attempt(&user_id.to_string()).await
    }
}
