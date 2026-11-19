use std::net::{IpAddr, Ipv4Addr};

use aurelin_fortify::{PASSKEY_ATTEMPTS_PER_MINUTE, PasskeyRateLimiter};
use aurelin_framework::auth::RateLimitOutcome;

#[tokio::test]
async fn ten_passkey_requests_are_allowed_per_identifier_and_address() {
    let limiter = PasskeyRateLimiter::default();
    let address = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let key = PasskeyRateLimiter::key("credential-or-session", address);

    for remaining in (0..PASSKEY_ATTEMPTS_PER_MINUTE).rev() {
        assert_eq!(limiter.attempt(&key).await, RateLimitOutcome::Allowed { remaining });
    }
    assert!(matches!(limiter.attempt(&key).await, RateLimitOutcome::Limited { .. }));
    assert!(matches!(
        limiter.attempt(&PasskeyRateLimiter::key("other-credential", address)).await,
        RateLimitOutcome::Allowed { remaining: 9 }
    ));
}
