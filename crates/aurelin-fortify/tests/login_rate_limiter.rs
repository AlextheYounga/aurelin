use std::{net::IpAddr, str::FromStr};

use aurelin_fortify::LoginRateLimiter;
use aurelin_framework::auth::RateLimitOutcome;

#[tokio::test]
async fn five_login_requests_block_the_same_normalized_email_and_address() {
    let limiter = LoginRateLimiter::default();
    let address = IpAddr::from_str("127.0.0.1").ok();
    assert!(address.is_some());
    let key = LoginRateLimiter::key("Ada@Example.com", address.unwrap_or(IpAddr::from([127, 0, 0, 1])));
    for remaining in (0..5).rev() {
        assert_eq!(limiter.attempt(&key).await, RateLimitOutcome::Allowed { remaining });
    }
    assert!(matches!(limiter.attempt(&key).await, RateLimitOutcome::Limited { .. }));
    limiter.clear(&key).await;
    assert!(limiter.retry_after(&key).await.is_none());
}
