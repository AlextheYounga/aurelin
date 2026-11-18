use std::{net::IpAddr, time::Duration};

use aurelin_framework::auth::FixedWindowRateLimiter;

#[derive(Clone, Debug)]
pub struct LoginRateLimiter {
    limiter: FixedWindowRateLimiter,
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self { limiter: FixedWindowRateLimiter::new(5, Duration::from_secs(60)) }
    }
}

impl LoginRateLimiter {
    pub fn key(email: &str, address: IpAddr) -> String {
        format!("{}|{address}", email.to_lowercase())
    }

    pub async fn retry_after(&self, key: &str) -> Option<Duration> {
        self.limiter.retry_after(key).await
    }

    pub async fn hit(&self, key: &str) {
        self.limiter.hit(key).await;
    }

    pub async fn clear(&self, key: &str) {
        self.limiter.clear(key).await;
    }
}
