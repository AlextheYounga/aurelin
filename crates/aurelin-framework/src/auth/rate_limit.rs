use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct FixedWindowRateLimiter {
    limit: u32,
    window: Duration,
    entries: Arc<Mutex<HashMap<String, Window>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitOutcome {
    Allowed { remaining: u32 },
    Limited { retry_after: Duration },
}

#[derive(Clone, Copy, Debug)]
struct Window {
    hits: u32,
    expires_at: Instant,
}

impl FixedWindowRateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self { limit, window, entries: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub async fn retry_after(&self, key: &str) -> Option<Duration> {
        let now = Instant::now();
        let mut entries = self.entries.lock().await;
        entries.retain(|_, window| window.expires_at > now);
        let window = entries.get(key)?;
        (window.hits >= self.limit).then(|| window.expires_at.saturating_duration_since(now))
    }

    pub async fn attempt(&self, key: &str) -> RateLimitOutcome {
        let now = Instant::now();
        let mut entries = self.entries.lock().await;
        entries.retain(|_, window| window.expires_at > now);

        let window = entries.entry(key.to_owned()).or_insert(Window { hits: 0, expires_at: now + self.window });
        if window.hits >= self.limit {
            return RateLimitOutcome::Limited { retry_after: window.expires_at.saturating_duration_since(now) };
        }

        window.hits += 1;
        RateLimitOutcome::Allowed { remaining: self.limit - window.hits }
    }

    pub async fn hit(&self, key: &str) {
        let now = Instant::now();
        let mut entries = self.entries.lock().await;
        entries.retain(|_, window| window.expires_at > now);
        entries
            .entry(key.to_owned())
            .and_modify(|window| window.hits += 1)
            .or_insert(Window { hits: 1, expires_at: now + self.window });
    }

    pub async fn clear(&self, key: &str) {
        self.entries.lock().await.remove(key);
    }
}
