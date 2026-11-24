//! Laravel-inspired typed caching over process-local Moka and distributed Redis stores.

mod error;
mod local;
mod lock;
mod redis;
mod store;

use std::{future::Future, sync::Arc, time::Duration};

use serde::{Serialize, de::DeserializeOwned};

pub use error::{Error, RememberError};
pub use local::LocalStore;
pub use lock::Lock;
pub use redis::RedisStore;
pub use store::Store;

/// Cloneable application-facing cache API.
#[derive(Clone)]
pub struct Cache {
    store: Arc<dyn Store>,
    prefix: Arc<str>,
}

impl Cache {
    /// Wrap a cache driver and apply `prefix` to item and lock names.
    pub fn new(store: impl Store, prefix: impl Into<String>) -> Self {
        Self { store: Arc::new(store), prefix: Arc::from(prefix.into()) }
    }

    /// Retrieve and decode a value, returning `None` when the key is absent.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        self.store.get(&self.key(key)).await?.map(|value| decode(key, &value)).transpose()
    }

    /// Retrieve several values in input order.
    pub async fn get_many<T: DeserializeOwned>(&self, keys: &[&str]) -> Result<Vec<Option<T>>, Error> {
        let mut values = Vec::with_capacity(keys.len());
        for key in keys {
            values.push(self.get(key).await?);
        }
        Ok(values)
    }

    /// Store a value for a signed number of seconds.
    ///
    /// A zero or negative lifetime removes the key instead.
    pub async fn put<T: Serialize>(&self, key: &str, value: &T, seconds: i64) -> Result<(), Error> {
        if seconds <= 0 {
            self.forget(key).await?;
            return Ok(());
        }

        self.store.put(&self.key(key), encode(key, value)?, Some(Duration::from_secs(seconds.unsigned_abs()))).await
    }

    /// Store several values with one lifetime.
    pub async fn put_many<T: Serialize>(&self, values: &[(String, T)], seconds: i64) -> Result<(), Error> {
        let encoded = values
            .iter()
            .map(|(key, value)| encode(key, value).map(|value| (key, value)))
            .collect::<Result<Vec<_>, _>>()?;

        for (key, value) in encoded {
            if seconds <= 0 {
                self.forget(key).await?;
            } else {
                self.store.put(&self.key(key), value, Some(Duration::from_secs(seconds.unsigned_abs()))).await?;
            }
        }
        Ok(())
    }

    /// Store a value without expiration.
    pub async fn forever<T: Serialize>(&self, key: &str, value: &T) -> Result<(), Error> {
        self.store.put(&self.key(key), encode(key, value)?, None).await
    }

    /// Store a value only when the key is absent.
    ///
    /// `None` stores indefinitely. A zero or negative lifetime performs no write.
    pub async fn add<T: Serialize>(&self, key: &str, value: &T, seconds: Option<i64>) -> Result<bool, Error> {
        let ttl = match seconds {
            Some(seconds) if seconds <= 0 => return Ok(false),
            Some(seconds) => Some(Duration::from_secs(seconds.unsigned_abs())),
            None => None,
        };
        self.store.add(&self.key(key), encode(key, value)?, ttl).await
    }

    /// Return and remove a value atomically.
    pub async fn pull<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        self.store.pull(&self.key(key)).await?.map(|value| decode(key, &value)).transpose()
    }

    /// Determine whether a key exists.
    pub async fn has(&self, key: &str) -> Result<bool, Error> {
        Ok(self.store.get(&self.key(key)).await?.is_some())
    }

    /// Determine whether a key does not exist.
    pub async fn missing(&self, key: &str) -> Result<bool, Error> {
        Ok(!self.has(key).await?)
    }

    /// Atomically increment an integer, creating an indefinite value when absent.
    pub async fn increment(&self, key: &str, amount: i64) -> Result<i64, Error> {
        self.store.increment(&self.key(key), amount).await
    }

    /// Atomically decrement an integer, creating an indefinite value when absent.
    pub async fn decrement(&self, key: &str, amount: i64) -> Result<i64, Error> {
        let amount = amount.checked_neg().ok_or_else(|| Error::IntegerOverflow(key.to_owned()))?;
        self.store.increment(&self.key(key), amount).await
    }

    /// Replace an existing key's lifetime, or remove it for a non-positive lifetime.
    pub async fn touch(&self, key: &str, seconds: i64) -> Result<bool, Error> {
        if seconds <= 0 {
            return self.forget(key).await;
        }
        self.store.touch(&self.key(key), Duration::from_secs(seconds.unsigned_abs())).await
    }

    /// Remove one item, returning whether it existed.
    pub async fn forget(&self, key: &str) -> Result<bool, Error> {
        self.store.forget(&self.key(key)).await
    }

    /// Remove all entries from the selected backend, ignoring this handle's prefix.
    pub async fn flush(&self) -> Result<(), Error> {
        self.store.flush().await
    }

    /// Retrieve a value or resolve and cache it for the given lifetime.
    pub async fn remember<T, F, Fut, E>(&self, key: &str, seconds: i64, resolve: F) -> Result<T, RememberError<E>>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let value = resolve().await.map_err(RememberError::Resolve)?;
        self.put(key, &value, seconds).await?;
        Ok(value)
    }

    /// Retrieve a value or resolve and cache it without expiration.
    pub async fn remember_forever<T, F, Fut, E>(&self, key: &str, resolve: F) -> Result<T, RememberError<E>>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let value = resolve().await.map_err(RememberError::Resolve)?;
        self.forever(key, &value).await?;
        Ok(value)
    }

    /// Create a lock with a random owner token. Non-positive seconds means no expiration.
    pub fn lock(&self, name: &str, seconds: i64) -> Lock {
        Lock::new(Arc::clone(&self.store), self.key(name), seconds)
    }

    /// Restore a lock handle using an owner token from another process.
    pub fn restore_lock(&self, name: &str, owner: impl Into<String>) -> Lock {
        Lock::restored(Arc::clone(&self.store), self.key(name), owner.into())
    }

    /// Return this handle's explicit key prefix.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn key(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }
}

fn encode(key: &str, value: &impl Serialize) -> Result<Vec<u8>, Error> {
    let encoded = serde_json::to_vec(value).map_err(|source| Error::Encode { key: key.to_owned(), source })?;
    if encoded == b"null" {
        return Err(Error::NullValue(key.to_owned()));
    }
    Ok(encoded)
}

fn decode<T: DeserializeOwned>(key: &str, value: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(value).map_err(|source| Error::Decode { key: key.to_owned(), source })
}
