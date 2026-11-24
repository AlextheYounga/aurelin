use std::{
    convert::Infallible,
    error::Error as StdError,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use aurelin_framework::cache::{Cache, Error};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinSet, time::sleep};

type TestResult = Result<(), Box<dyn StdError>>;

pub fn expected_cache_error<T>(result: Result<T, Error>, message: &str) -> Result<Error, io::Error> {
    match result {
        Ok(_) => Err(io::Error::other(message)),
        Err(error) => Ok(error),
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Profile {
    name: String,
    roles: Vec<String>,
}

pub async fn basic_operations(cache: &Cache) -> TestResult {
    assert_eq!(cache.get::<String>("missing").await?, None);
    assert!(cache.missing("missing").await?);
    assert!(!cache.has("missing").await?);

    cache.put("greeting", &"hello", 30).await?;
    assert_eq!(cache.get::<String>("greeting").await?, Some("hello".to_owned()));
    assert!(cache.has("greeting").await?);

    cache.put("greeting", &"goodbye", 30).await?;
    assert_eq!(cache.get::<String>("greeting").await?, Some("goodbye".to_owned()));
    assert!(cache.forget("greeting").await?);
    assert!(!cache.forget("greeting").await?);

    cache.forever("permanent", &42_i64).await?;
    assert_eq!(cache.get::<i64>("permanent").await?, Some(42));
    Ok(())
}

pub async fn expiration_and_conditional_writes(cache: &Cache) -> TestResult {
    cache.put("expires", &"soon", 1).await?;
    sleep(Duration::from_millis(1_100)).await;
    assert_eq!(cache.get::<String>("expires").await?, None);

    cache.forever("zero", &"existing").await?;
    cache.put("zero", &"replacement", 0).await?;
    assert!(cache.missing("zero").await?);

    cache.forever("negative", &"existing").await?;
    cache.put("negative", &"replacement", -1).await?;
    assert!(cache.missing("negative").await?);

    assert!(cache.add("unique", &1_i64, Some(30)).await?);
    assert!(!cache.add("unique", &2_i64, Some(30)).await?);
    assert_eq!(cache.get::<i64>("unique").await?, Some(1));
    assert!(!cache.add("never-written", &1_i64, Some(0)).await?);
    assert!(cache.missing("never-written").await?);
    Ok(())
}

pub async fn compound_operations(cache: &Cache) -> TestResult {
    cache.forever("pulled", &vec![1_i64, 2, 3]).await?;
    assert_eq!(cache.pull::<Vec<i64>>("pulled").await?, Some(vec![1, 2, 3]));
    assert_eq!(cache.pull::<Vec<i64>>("pulled").await?, None);

    assert_eq!(cache.increment("counter", 3).await?, 3);
    assert_eq!(cache.increment("counter", 2).await?, 5);
    assert_eq!(cache.decrement("counter", 4).await?, 1);
    assert_eq!(cache.decrement("new-counter", 2).await?, -2);

    let computed = Arc::new(AtomicUsize::new(0));
    let first_count = Arc::clone(&computed);
    let first: String = cache
        .remember("remembered", 30, || async move {
            first_count.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>("resolved".to_owned())
        })
        .await?;
    let second_count = Arc::clone(&computed);
    let second: String = cache
        .remember("remembered", 30, || async move {
            second_count.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>("wrong".to_owned())
        })
        .await?;
    assert_eq!(first, "resolved");
    assert_eq!(second, "resolved");
    assert_eq!(computed.load(Ordering::SeqCst), 1);

    let permanent: String =
        cache.remember_forever("remembered-forever", || async { Ok::<_, Infallible>("permanent".to_owned()) }).await?;
    assert_eq!(permanent, "permanent");

    cache.forever("touched", &true).await?;
    assert!(cache.touch("touched", 30).await?);
    assert!(!cache.touch("absent", 30).await?);

    cache.put_many(&[("one".to_owned(), 1_i64), ("two".to_owned(), 2_i64)], 30).await?;
    assert_eq!(cache.get_many::<i64>(&["one", "absent", "two"]).await?, vec![Some(1), None, Some(2)]);
    Ok(())
}

pub async fn serialization(cache: &Cache) -> TestResult {
    let profile = Profile { name: "Taylor".to_owned(), roles: vec!["admin".to_owned(), "author".to_owned()] };
    cache.forever("profile", &profile).await?;
    assert_eq!(cache.get::<Profile>("profile").await?, Some(profile));

    let error = expected_cache_error(cache.get::<u64>("profile").await, "a type mismatch must not be a miss")?;
    assert!(matches!(error, Error::Decode { .. }));

    let error = expected_cache_error(cache.forever("null", &Option::<u8>::None).await, "JSON null is ambiguous")?;
    assert!(matches!(error, Error::NullValue(_)));
    Ok(())
}

pub async fn cloned_handles_and_atomic_add(cache: &Cache) -> TestResult {
    let clone = cache.clone();
    cache.forever("shared", &"state").await?;
    assert_eq!(clone.get::<String>("shared").await?, Some("state".to_owned()));

    let mut tasks = JoinSet::new();
    for contender in 0_i64..16 {
        let cache = cache.clone();
        tasks.spawn(async move { cache.add("contended", &contender, Some(30)).await });
    }

    let mut winners = 0;
    while let Some(result) = tasks.join_next().await {
        if result?? {
            winners += 1;
        }
    }
    assert_eq!(winners, 1);
    Ok(())
}

pub async fn locks(cache: &Cache) -> TestResult {
    let first = cache.lock("report", 1);
    let second = cache.lock("report", 1);
    assert_ne!(first.owner(), second.owner());
    assert!(first.acquire().await?);
    assert!(!second.acquire().await?);
    assert!(first.is_owned().await?);
    assert!(!second.is_owned().await?);
    assert!(!second.release().await?);
    assert!(!second.refresh(Some(30)).await?);
    assert!(first.refresh(Some(2)).await?);

    let restored = cache.restore_lock("report", first.owner());
    assert!(restored.is_owned().await?);
    assert!(restored.release().await?);
    assert!(second.acquire().await?);
    assert!(second.release().await?);

    let expiring = cache.lock("expiring", 1);
    assert!(expiring.acquire().await?);
    sleep(Duration::from_millis(1_100)).await;
    assert!(cache.lock("expiring", 1).acquire().await?);

    let held = cache.lock("blocked", 30);
    assert!(held.acquire().await?);
    let waiting = cache.lock("blocked", 30);
    let started = Instant::now();
    let error = expected_cache_error(waiting.block(Duration::from_millis(120)).await, "held lock should time out")?;
    assert!(matches!(error, Error::LockTimeout(_)));
    assert!(started.elapsed() >= Duration::from_millis(100));
    held.force_release().await?;
    Ok(())
}
