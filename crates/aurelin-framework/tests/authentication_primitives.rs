use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};

use aurelin_framework::{
    auth::{
        BcryptPasswordHasher, FixedWindowRateLimiter, PasswordResetBroker, PasswordResetCredentials,
        PasswordResetRepository, PasswordResetRequest, RateLimitOutcome, StoredPasswordResetToken,
    },
    signed_url::TemporarySignedUrl,
};

#[tokio::test]
async fn bcrypt_uses_laravels_default_cost_and_verifies_passwords() {
    let passwords = BcryptPasswordHasher::default();
    let password_hash = passwords.hash("correct horse battery staple").await;
    assert!(password_hash.is_ok());
    let password_hash = password_hash.ok().unwrap_or_default();
    assert!(password_hash.starts_with("$2b$12$"));
    assert_eq!(passwords.verify("correct horse battery staple", &password_hash).await.ok(), Some(true));
    assert_eq!(passwords.verify("incorrect", &password_hash).await.ok(), Some(false));
    assert_eq!(passwords.needs_rehash(&password_hash).ok(), Some(false));

    let lower_cost_hash = BcryptPasswordHasher::new(4).hash("correct horse battery staple").await;
    assert!(lower_cost_hash.is_ok());
    assert_eq!(lower_cost_hash.ok().and_then(|hash| passwords.needs_rehash(&hash).ok()), Some(true));
}

#[tokio::test]
async fn fixed_windows_block_after_the_configured_number_of_hits_and_can_be_cleared() {
    let limiter = FixedWindowRateLimiter::new(2, Duration::from_secs(60));
    assert_eq!(limiter.attempt("login").await, RateLimitOutcome::Allowed { remaining: 1 });
    assert_eq!(limiter.attempt("login").await, RateLimitOutcome::Allowed { remaining: 0 });
    assert!(matches!(limiter.attempt("login").await, RateLimitOutcome::Limited { .. }));
    limiter.clear("login").await;
    assert!(limiter.retry_after("login").await.is_none());
}

#[test]
fn signed_urls_reject_modified_paths_and_signatures() -> Result<(), Box<dyn Error>> {
    let signer = TemporarySignedUrl::new(b"application-key")?;
    let path = "/email/verify/1/hash?expires=2000000000";
    let signature = signer.sign(path)?;
    assert_eq!(signer.verify(path, &signature).ok(), Some(true));
    assert_eq!(signer.verify("/email/verify/2/hash?expires=2000000000", &signature).ok(), Some(false));
    assert_eq!(signer.verify(path, "invalid").ok(), Some(false));
    Ok(())
}

#[tokio::test]
async fn password_broker_owns_token_throttling_expiration_and_consumption() {
    let repository = MemoryResetRepository::with_user("ada@example.com");
    let broker = PasswordResetBroker::new(repository.clone());
    let request = broker.create_token("ada@example.com", 1_000).await;
    let token = match request {
        Ok(PasswordResetRequest::Sent { token }) => token,
        _ => String::new(),
    };
    assert!(!token.is_empty());
    assert_eq!(broker.create_token("ada@example.com", 1_030).await.ok(), Some(PasswordResetRequest::Throttled));
    assert_eq!(broker.create_token("unknown@example.com", 1_030).await.ok(), Some(PasswordResetRequest::UnknownUser));
    let wrong = broker
        .validate_reset(
            PasswordResetCredentials { email: "ada@example.com", token: "wrong", password: "new-password" },
            1_040,
        )
        .await;
    assert!(matches!(wrong, Ok(None)));
    let expired = broker
        .validate_reset(
            PasswordResetCredentials { email: "ada@example.com", token: &token, password: "new-password" },
            4_601,
        )
        .await;
    assert!(matches!(expired, Ok(None)));
    let reset = broker
        .validate_reset(
            PasswordResetCredentials { email: "ada@example.com", token: &token, password: "new-password" },
            1_040,
        )
        .await
        .ok()
        .flatten();
    assert!(reset.is_some());
    if let Some(reset) = reset {
        assert_eq!(reset.password, "new-password");
        assert_eq!(broker.complete_reset(&reset).await.ok(), Some(true));
    }
    assert!(repository.token("ada@example.com").await.ok().flatten().is_none());
}

#[derive(Clone, Default)]
struct MemoryResetRepository {
    state: Arc<Mutex<MemoryResetState>>,
}

#[derive(Default)]
struct MemoryResetState {
    users: HashMap<String, String>,
    tokens: HashMap<String, StoredPasswordResetToken>,
}

impl MemoryResetRepository {
    fn with_user(email: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(MemoryResetState {
                users: HashMap::from([(email.to_owned(), String::new())]),
                tokens: HashMap::new(),
            })),
        }
    }
}

impl PasswordResetRepository for MemoryResetRepository {
    type Error = Infallible;

    async fn user_exists(&self, email: &str) -> Result<bool, Self::Error> {
        Ok(self.state.lock().is_ok_and(|state| state.users.contains_key(email)))
    }

    async fn token(&self, email: &str) -> Result<Option<StoredPasswordResetToken>, Self::Error> {
        Ok(self.state.lock().ok().and_then(|state| state.tokens.get(email).cloned()))
    }

    async fn replace_token(&self, email: &str, token: StoredPasswordResetToken) -> Result<(), Self::Error> {
        if let Ok(mut state) = self.state.lock() {
            state.tokens.insert(email.to_owned(), token);
        }
        Ok(())
    }

    async fn delete_token(&self, email: &str, expected_digest: &str) -> Result<bool, Self::Error> {
        let Ok(mut state) = self.state.lock() else {
            return Ok(false);
        };
        if !state.tokens.get(email).is_some_and(|token| token.digest == expected_digest) {
            return Ok(false);
        }
        state.tokens.remove(email);
        Ok(true)
    }
}
