mod support;

use std::{error::Error, time::Duration};

use aurelin_framework::cache::{Cache, Error as CacheError, LocalStore, Store};
use tokio::time::sleep;

fn local_cache(prefix: &str) -> Cache {
    Cache::new(LocalStore::new(10_000), prefix)
}

#[tokio::test]
async fn local_cache_supports_core_repository_operations() -> Result<(), Box<dyn Error>> {
    let cache = local_cache("basic:");
    support::cache::basic_operations(&cache).await?;
    support::cache::compound_operations(&cache).await?;
    Ok(())
}

#[tokio::test]
async fn local_cache_applies_expiration_and_conditional_writes() -> Result<(), Box<dyn Error>> {
    support::cache::expiration_and_conditional_writes(&local_cache("ttl:")).await
}

#[tokio::test]
async fn local_cache_serializes_typed_values_and_reports_bad_data() -> Result<(), Box<dyn Error>> {
    let store = LocalStore::new(100);
    let cache = Cache::new(store.clone(), "json:");
    support::cache::serialization(&cache).await?;

    store.put("json:corrupt", b"{bad json".to_vec(), None).await?;
    let error =
        support::cache::expected_cache_error(cache.get::<String>("corrupt").await, "corrupt data must be reported")?;
    assert!(matches!(error, CacheError::Decode { .. }));
    Ok(())
}

#[tokio::test]
async fn local_cache_clones_share_state_and_add_is_atomic() -> Result<(), Box<dyn Error>> {
    support::cache::cloned_handles_and_atomic_add(&local_cache("atomic:")).await
}

#[tokio::test]
async fn local_cache_flush_ignores_prefixes() -> Result<(), Box<dyn Error>> {
    let store = LocalStore::new(100);
    let first = Cache::new(store.clone(), "first:");
    let second = Cache::new(store, "second:");
    first.forever("key", &1_i64).await?;
    second.forever("key", &2_i64).await?;

    first.flush().await?;
    assert!(first.missing("key").await?);
    assert!(second.missing("key").await?);
    Ok(())
}

#[tokio::test]
async fn local_integer_mutation_preserves_ttl_and_errors_without_changing_values() -> Result<(), Box<dyn Error>> {
    let cache = local_cache("integer:");
    cache.put("expiring", &1_i64, 1).await?;
    sleep(Duration::from_millis(600)).await;
    assert_eq!(cache.increment("expiring", 1).await?, 2);
    sleep(Duration::from_millis(550)).await;
    assert!(cache.missing("expiring").await?);

    cache.forever("maximum", &i64::MAX).await?;
    let error = support::cache::expected_cache_error(cache.increment("maximum", 1).await, "overflow must fail")?;
    assert!(matches!(error, CacheError::IntegerOverflow(_)));
    assert_eq!(cache.get::<i64>("maximum").await?, Some(i64::MAX));

    cache.forever("text", &"not a number").await?;
    let error = support::cache::expected_cache_error(cache.increment("text", 1).await, "non-integers must fail")?;
    assert!(matches!(error, CacheError::NotInteger { .. }));
    Ok(())
}

#[tokio::test]
async fn local_locks_enforce_owner_expiration_restoration_and_timeouts() -> Result<(), Box<dyn Error>> {
    support::cache::locks(&local_cache("lock:")).await
}
