use std::time::Duration;

use async_trait::async_trait;

use super::Error;

/// Driver contract used by Aurelin's typed cache API.
#[async_trait]
pub trait Store: Send + Sync + 'static {
    /// Retrieve raw encoded data.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;

    /// Store raw encoded data with an optional lifetime.
    async fn put(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), Error>;

    /// Store data only when the key is absent.
    async fn add(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<bool, Error>;

    /// Remove and return raw encoded data atomically.
    async fn pull(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;

    /// Atomically mutate an integer while retaining an existing lifetime.
    async fn increment(&self, key: &str, amount: i64) -> Result<i64, Error>;

    /// Replace the lifetime of an existing item.
    async fn touch(&self, key: &str, ttl: Duration) -> Result<bool, Error>;

    /// Remove one item, returning whether it existed.
    async fn forget(&self, key: &str) -> Result<bool, Error>;

    /// Remove all items from the selected backend, regardless of key prefix.
    async fn flush(&self) -> Result<(), Error>;

    /// Atomically acquire a lock for an owner.
    async fn acquire_lock(&self, key: &str, owner: &str, ttl: Option<Duration>) -> Result<bool, Error>;

    /// Atomically release a lock only when its owner matches.
    async fn release_lock(&self, key: &str, owner: &str) -> Result<bool, Error>;

    /// Release a lock without checking its owner.
    async fn force_release_lock(&self, key: &str) -> Result<(), Error>;

    /// Atomically replace the lifetime only when the owner matches.
    async fn refresh_lock(&self, key: &str, owner: &str, ttl: Option<Duration>) -> Result<bool, Error>;

    /// Return the current lock owner, if the lock exists.
    async fn lock_owner(&self, key: &str) -> Result<Option<String>, Error>;
}
