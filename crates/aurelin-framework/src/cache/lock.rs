use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use tokio::time::{Instant, sleep};

use super::{Error, Store};

const RETRY_DELAY: Duration = Duration::from_millis(250);

/// An owner-scoped atomic cache lock.
#[derive(Clone)]
pub struct Lock {
    store: Arc<dyn Store>,
    key: String,
    owner: String,
    ttl: Option<Duration>,
}

impl Lock {
    pub(super) fn new(store: Arc<dyn Store>, key: String, seconds: i64) -> Self {
        Self { store, key, owner: random_owner(), ttl: positive_duration(seconds) }
    }

    pub(super) fn restored(store: Arc<dyn Store>, key: String, owner: String) -> Self {
        Self { store, key, owner, ttl: None }
    }

    /// Attempt to acquire this lock immediately.
    pub async fn acquire(&self) -> Result<bool, Error> {
        self.store.acquire_lock(&self.key, &self.owner, self.ttl).await
    }

    /// Wait up to `timeout` for this lock to become available.
    pub async fn block(&self, timeout: Duration) -> Result<(), Error> {
        let deadline = Instant::now() + timeout;

        loop {
            if self.acquire().await? {
                return Ok(());
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(Error::LockTimeout(timeout));
            }

            sleep(RETRY_DELAY.min(deadline.saturating_duration_since(now))).await;
        }
    }

    /// Release this lock if its stored owner still matches.
    pub async fn release(&self) -> Result<bool, Error> {
        self.store.release_lock(&self.key, &self.owner).await
    }

    /// Release the named lock regardless of its current owner.
    pub async fn force_release(&self) -> Result<(), Error> {
        self.store.force_release_lock(&self.key).await
    }

    /// Refresh the lock using another signed-second lifetime.
    ///
    /// Zero or negative seconds make an owned lock indefinite.
    pub async fn refresh(&self, seconds: Option<i64>) -> Result<bool, Error> {
        let ttl = seconds.map_or(self.ttl, positive_duration);
        self.store.refresh_lock(&self.key, &self.owner, ttl).await
    }

    /// Return the opaque owner token used by this handle.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Determine whether this handle still owns the stored lock.
    pub async fn is_owned(&self) -> Result<bool, Error> {
        Ok(self.store.lock_owner(&self.key).await?.as_deref() == Some(&self.owner))
    }

    /// Determine whether any owner currently holds the lock.
    pub async fn is_locked(&self) -> Result<bool, Error> {
        Ok(self.store.lock_owner(&self.key).await?.is_some())
    }
}

fn positive_duration(seconds: i64) -> Option<Duration> {
    u64::try_from(seconds).ok().filter(|seconds| *seconds > 0).map(Duration::from_secs)
}

fn random_owner() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
