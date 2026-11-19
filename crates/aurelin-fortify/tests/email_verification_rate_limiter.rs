use aurelin_fortify::EmailVerificationRateLimiter;
use aurelin_framework::auth::RateLimitOutcome;

#[tokio::test]
async fn six_verification_requests_are_allowed_per_user_each_minute() {
    let limiter = EmailVerificationRateLimiter::default();

    for remaining in (0..6).rev() {
        assert_eq!(limiter.attempt(7).await, RateLimitOutcome::Allowed { remaining });
    }
    assert!(matches!(limiter.attempt(7).await, RateLimitOutcome::Limited { .. }));
    assert_eq!(limiter.attempt(8).await, RateLimitOutcome::Allowed { remaining: 5 });
}
