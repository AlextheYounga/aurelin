use std::{
    future::ready,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use moka::{
    Expiry,
    future::Cache as MokaCache,
    ops::compute::{CompResult, Op},
};

use super::{Error, Store};

/// Process-local cache driver backed by Moka.
#[derive(Clone)]
pub struct LocalStore {
    values: MokaCache<String, StoredValue>,
    locks: MokaCache<String, LockValue>,
}

impl LocalStore {
    /// Create a process-local store with separate value and lock capacities.
    pub fn new(max_capacity: u64) -> Self {
        Self {
            values: MokaCache::builder().max_capacity(max_capacity).expire_after(ValueExpiry).build(),
            locks: MokaCache::builder().max_capacity(max_capacity).expire_after(LockExpiry).build(),
        }
    }
}

#[async_trait]
impl Store for LocalStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.values.get(key).await.map(|value| value.data))
    }

    async fn put(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), Error> {
        self.values.insert(key.to_owned(), StoredValue { data: value, expiry: EntryExpiry::Reset(ttl) }).await;
        Ok(())
    }

    async fn add(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<bool, Error> {
        let entry = self
            .values
            .entry(key.to_owned())
            .or_insert(StoredValue { data: value, expiry: EntryExpiry::Reset(ttl) })
            .await;
        Ok(entry.is_fresh())
    }

    async fn pull(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let result = self
            .values
            .entry(key.to_owned())
            .and_compute_with(|entry| ready(if entry.is_some() { Op::Remove } else { Op::Nop }))
            .await;
        Ok(match result {
            CompResult::Removed(entry) => Some(entry.into_value().data),
            _ => None,
        })
    }

    async fn increment(&self, key: &str, amount: i64) -> Result<i64, Error> {
        let logical_key = key.to_owned();
        let result = self
            .values
            .entry(logical_key.clone())
            .and_try_compute_with(|entry| {
                let result = match entry {
                    Some(entry) => decode_integer(&logical_key, &entry.into_value().data)
                        .and_then(|current| checked_add(&logical_key, current, amount))
                        .and_then(|value| integer_value(&logical_key, value, EntryExpiry::Keep)),
                    None => integer_value(&logical_key, amount, EntryExpiry::Reset(None)),
                }
                .map(Op::Put);
                ready(result)
            })
            .await?;

        let entry = result.into_entry().ok_or_else(|| Error::NotInteger { key: logical_key.clone() })?;
        decode_integer(&logical_key, &entry.into_value().data)
    }

    async fn touch(&self, key: &str, ttl: Duration) -> Result<bool, Error> {
        let result = self
            .values
            .entry(key.to_owned())
            .and_compute_with(|entry| {
                ready(match entry {
                    Some(entry) => {
                        Op::Put(StoredValue { data: entry.into_value().data, expiry: EntryExpiry::Reset(Some(ttl)) })
                    }
                    None => Op::Nop,
                })
            })
            .await;
        Ok(matches!(result, CompResult::ReplacedWith(_)))
    }

    async fn forget(&self, key: &str) -> Result<bool, Error> {
        Ok(self.values.remove(key).await.is_some())
    }

    async fn flush(&self) -> Result<(), Error> {
        self.values.invalidate_all();
        self.values.run_pending_tasks().await;
        Ok(())
    }

    async fn acquire_lock(&self, key: &str, owner: &str, ttl: Option<Duration>) -> Result<bool, Error> {
        let entry = self.locks.entry(key.to_owned()).or_insert(LockValue { owner: owner.to_owned(), ttl }).await;
        Ok(entry.is_fresh())
    }

    async fn release_lock(&self, key: &str, owner: &str) -> Result<bool, Error> {
        let owner = owner.to_owned();
        let result = self
            .locks
            .entry(key.to_owned())
            .and_compute_with(|entry| {
                ready(match entry {
                    Some(entry) if entry.value().owner == owner => Op::Remove,
                    _ => Op::Nop,
                })
            })
            .await;
        Ok(matches!(result, CompResult::Removed(_)))
    }

    async fn force_release_lock(&self, key: &str) -> Result<(), Error> {
        self.locks.invalidate(key).await;
        Ok(())
    }

    async fn refresh_lock(&self, key: &str, owner: &str, ttl: Option<Duration>) -> Result<bool, Error> {
        let owner = owner.to_owned();
        let result = self
            .locks
            .entry(key.to_owned())
            .and_compute_with(|entry| {
                ready(match entry {
                    Some(entry) if entry.value().owner == owner => Op::Put(LockValue { owner, ttl }),
                    _ => Op::Nop,
                })
            })
            .await;
        Ok(matches!(result, CompResult::ReplacedWith(_)))
    }

    async fn lock_owner(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self.locks.get(key).await.map(|lock| lock.owner))
    }
}

#[derive(Clone)]
struct StoredValue {
    data: Vec<u8>,
    expiry: EntryExpiry,
}

#[derive(Clone, Copy)]
enum EntryExpiry {
    Reset(Option<Duration>),
    Keep,
}

struct ValueExpiry;

impl Expiry<String, StoredValue> for ValueExpiry {
    fn expire_after_create(&self, _key: &String, value: &StoredValue, _created_at: Instant) -> Option<Duration> {
        match value.expiry {
            EntryExpiry::Reset(ttl) => ttl,
            EntryExpiry::Keep => None,
        }
    }

    fn expire_after_update(
        &self,
        _key: &String,
        value: &StoredValue,
        _updated_at: Instant,
        current: Option<Duration>,
    ) -> Option<Duration> {
        match value.expiry {
            EntryExpiry::Reset(ttl) => ttl,
            EntryExpiry::Keep => current,
        }
    }
}

#[derive(Clone)]
struct LockValue {
    owner: String,
    ttl: Option<Duration>,
}

struct LockExpiry;

impl Expiry<String, LockValue> for LockExpiry {
    fn expire_after_create(&self, _key: &String, value: &LockValue, _created_at: Instant) -> Option<Duration> {
        value.ttl
    }

    fn expire_after_update(
        &self,
        _key: &String,
        value: &LockValue,
        _updated_at: Instant,
        _current: Option<Duration>,
    ) -> Option<Duration> {
        value.ttl
    }
}

fn decode_integer(key: &str, value: &[u8]) -> Result<i64, Error> {
    serde_json::from_slice(value).map_err(|_| Error::NotInteger { key: key.to_owned() })
}

fn checked_add(key: &str, current: i64, amount: i64) -> Result<i64, Error> {
    current.checked_add(amount).ok_or_else(|| Error::IntegerOverflow(key.to_owned()))
}

fn integer_value(key: &str, value: i64, expiry: EntryExpiry) -> Result<StoredValue, Error> {
    Ok(StoredValue {
        data: serde_json::to_vec(&value).map_err(|source| Error::Encode { key: key.to_owned(), source })?,
        expiry,
    })
}
