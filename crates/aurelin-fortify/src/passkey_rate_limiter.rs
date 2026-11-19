use std::{net::IpAddr, time::Duration};

use aurelin_framework::auth::{FixedWindowRateLimiter, RateLimitOutcome};

pub const PASSKEY_ATTEMPTS_PER_MINUTE: u32 = 10;

#[derive(Clone, Debug)]
pub struct PasskeyRateLimiter {
    limiter: FixedWindowRateLimiter,
}

impl Default for PasskeyRateLimiter {
    fn default() -> Self {
        Self { limiter: FixedWindowRateLimiter::new(PASSKEY_ATTEMPTS_PER_MINUTE, Duration::from_secs(60)) }
    }
}

impl PasskeyRateLimiter {
    pub fn key(identifier: &str, address: IpAddr) -> String {
        format!("{identifier}|{address}")
    }

    pub async fn attempt(&self, key: &str) -> RateLimitOutcome {
        self.limiter.attempt(key).await
    }
}
