mod support;

use std::{env, error::Error};

use aurelin_framework::cache::{Cache, RedisStore, Store};

#[tokio::test]
async fn redis_cache_and_locks_match_the_supported_contract() -> Result<(), Box<dyn Error>> {
    let Some(url) = env::var_os("AURELIN_TEST_REDIS_URL") else {
        return Ok(());
    };
    let url = url.to_string_lossy();
    let store = RedisStore::connect(&url).await?;
    let cache = Cache::new(store.clone(), "aurelin-contract:");
    cache.flush().await?;

    support::cache::basic_operations(&cache).await?;
    support::cache::expiration_and_conditional_writes(&cache).await?;
    support::cache::compound_operations(&cache).await?;
    support::cache::serialization(&cache).await?;
    support::cache::cloned_handles_and_atomic_add(&cache).await?;
    support::cache::locks(&cache).await?;

    store.put("aurelin-contract:corrupt", b"{bad json".to_vec(), None).await?;
    assert!(cache.get::<String>("corrupt").await.is_err());

    let other_prefix = Cache::new(store, "aurelin-other:");
    other_prefix.forever("survives-prefix", &false).await?;
    cache.flush().await?;
    assert!(other_prefix.missing("survives-prefix").await?);
    Ok(())
}

#[tokio::test]
async fn invalid_redis_configuration_is_returned_as_a_driver_error() {
    let result = RedisStore::connect("not-a-redis-url").await;
    assert!(result.is_err());
}
