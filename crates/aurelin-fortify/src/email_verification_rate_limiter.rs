use std::time::Duration;

use aurelin_framework::auth::{FixedWindowRateLimiter, RateLimitOutcome};

pub const EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE: u32 = 6;

#[derive(Clone, Debug)]
pub struct EmailVerificationRateLimiter {
    limiter: FixedWindowRateLimiter,
}

impl Default for EmailVerificationRateLimiter {
    fn default() -> Self {
        Self { limiter: FixedWindowRateLimiter::new(EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE, Duration::from_secs(60)) }
    }
}

impl EmailVerificationRateLimiter {
    pub async fn attempt(&self, user_id: i64) -> RateLimitOutcome {
        self.limiter.attempt(&user_id.to_string()).await
    }
}
