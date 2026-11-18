use std::{net::IpAddr, str::FromStr};

use aurelin_fortify::LoginRateLimiter;

#[tokio::test]
async fn five_failed_logins_block_the_same_normalized_email_and_address() {
    let limiter = LoginRateLimiter::default();
    let address = IpAddr::from_str("127.0.0.1").ok();
    assert!(address.is_some());
    let key = LoginRateLimiter::key("Ada@Example.com", address.unwrap_or(IpAddr::from([127, 0, 0, 1])));
    for _ in 0..5 {
        assert!(limiter.retry_after(&key).await.is_none());
        limiter.hit(&key).await;
    }
    assert!(limiter.retry_after(&key).await.is_some());
    limiter.clear(&key).await;
    assert!(limiter.retry_after(&key).await.is_none());
}
