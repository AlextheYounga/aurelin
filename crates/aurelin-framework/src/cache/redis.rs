use std::time::Duration;

use async_trait::async_trait;
use fred::{
    bytes::Bytes,
    clients::Client,
    interfaces::{ClientLike, KeysInterface, LuaInterface},
    prelude::{Builder, Config},
    types::{ClusterHash, CustomCommand, Expiration, SetOptions},
};

use super::{Error, Store};

const RELEASE_LOCK: &str = include_str!("scripts/release_lock.lua");
const REFRESH_LOCK: &str = include_str!("scripts/refresh_lock.lua");

/// Distributed cache driver backed by Redis or Valkey through Fred.
#[derive(Clone)]
pub struct RedisStore {
    client: Client,
}

impl RedisStore {
    /// Configure and connect a Fred client from a Redis URL.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let config = Config::from_url(url)?;
        let client = Builder::from_config(config).build()?;
        client.init().await?;
        Ok(Self { client })
    }

    /// Use an already initialized Fred client.
    pub fn from_client(client: Client) -> Self {
        Self { client }
    }

    /// Borrow the underlying Fred client for service-level composition.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[async_trait]
impl Store for RedisStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let value: Option<Bytes> = self.client.get(key).await?;
        Ok(value.map(|value| value.to_vec()))
    }

    async fn put(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), Error> {
        let _: String = self.client.set(key, value, expiration(ttl), None, false).await?;
        Ok(())
    }

    async fn add(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<bool, Error> {
        let result: Option<String> = self.client.set(key, value, expiration(ttl), Some(SetOptions::NX), false).await?;
        Ok(result.is_some())
    }

    async fn pull(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let value: Option<Bytes> = self.client.getdel(key).await?;
        Ok(value.map(|value| value.to_vec()))
    }

    async fn increment(&self, key: &str, amount: i64) -> Result<i64, Error> {
        Ok(self.client.incr_by(key, amount).await?)
    }

    async fn touch(&self, key: &str, ttl: Duration) -> Result<bool, Error> {
        let result: i64 = self.client.pexpire(key, milliseconds(ttl), None).await?;
        Ok(result == 1)
    }

    async fn forget(&self, key: &str) -> Result<bool, Error> {
        let removed: i64 = self.client.del(key).await?;
        Ok(removed > 0)
    }

    async fn flush(&self) -> Result<(), Error> {
        let command = CustomCommand::new_static("FLUSHDB", ClusterHash::Random, false);
        let _: String = self.client.custom(command, Vec::<String>::new()).await?;
        Ok(())
    }

    async fn acquire_lock(&self, key: &str, owner: &str, ttl: Option<Duration>) -> Result<bool, Error> {
        let result: Option<String> = self.client.set(key, owner, expiration(ttl), Some(SetOptions::NX), false).await?;
        Ok(result.is_some())
    }

    async fn release_lock(&self, key: &str, owner: &str) -> Result<bool, Error> {
        let released: i64 = self.client.eval(RELEASE_LOCK, vec![key], vec![owner]).await?;
        Ok(released == 1)
    }

    async fn force_release_lock(&self, key: &str) -> Result<(), Error> {
        let _: i64 = self.client.del(key).await?;
        Ok(())
    }

    async fn refresh_lock(&self, key: &str, owner: &str, ttl: Option<Duration>) -> Result<bool, Error> {
        let ttl = ttl.map_or(0, milliseconds).to_string();
        let refreshed: i64 = self.client.eval(REFRESH_LOCK, vec![key], vec![owner, &ttl]).await?;
        Ok(refreshed == 1)
    }

    async fn lock_owner(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self.client.get(key).await?)
    }
}

fn expiration(ttl: Option<Duration>) -> Option<Expiration> {
    ttl.map(|ttl| Expiration::PX(milliseconds(ttl)))
}

fn milliseconds(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX).max(1)
}
