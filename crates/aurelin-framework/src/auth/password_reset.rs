use std::{error::Error, future::Future};

use rand::{Rng, distr::Alphanumeric};
use thiserror::Error;

use super::{BcryptPasswordHasher, PasswordHashError, digest_token};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredPasswordResetToken {
    pub digest: String,
    pub created_at: i64,
}

pub struct ConsumedPasswordReset<'a> {
    pub email: &'a str,
    pub expected_digest: &'a str,
    pub password_hash: &'a str,
    pub updated_at: i64,
}

pub struct PasswordResetCredentials<'a> {
    pub email: &'a str,
    pub token: &'a str,
    pub password: &'a str,
}

pub trait PasswordResetRepository: Clone + Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn user_exists(&self, email: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn token(&self, email: &str) -> impl Future<Output = Result<Option<StoredPasswordResetToken>, Self::Error>> + Send;
    fn replace_token(
        &self,
        email: &str,
        token: StoredPasswordResetToken,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn consume_token(&self, reset: ConsumedPasswordReset<'_>)
    -> impl Future<Output = Result<bool, Self::Error>> + Send;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasswordResetRequest {
    Sent { token: String },
    UnknownUser,
    Throttled,
}

#[derive(Clone, Debug)]
pub struct PasswordResetBroker<R> {
    repository: R,
    passwords: BcryptPasswordHasher,
    expires_after_seconds: i64,
    throttle_seconds: i64,
}

impl<R> PasswordResetBroker<R>
where
    R: PasswordResetRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            passwords: BcryptPasswordHasher::default(),
            expires_after_seconds: 60 * 60,
            throttle_seconds: 60,
        }
    }

    pub async fn create_token(
        &self,
        email: &str,
        now: i64,
    ) -> Result<PasswordResetRequest, PasswordResetError<R::Error>> {
        if !self.repository.user_exists(email).await.map_err(PasswordResetError::Repository)? {
            return Ok(PasswordResetRequest::UnknownUser);
        }
        if self
            .repository
            .token(email)
            .await
            .map_err(PasswordResetError::Repository)?
            .is_some_and(|token| token.created_at > now - self.throttle_seconds)
        {
            return Ok(PasswordResetRequest::Throttled);
        }
        let token: String = rand::rng().sample_iter(Alphanumeric).take(64).map(char::from).collect();
        self.repository
            .replace_token(email, StoredPasswordResetToken { digest: digest_token(&token), created_at: now })
            .await
            .map_err(PasswordResetError::Repository)?;
        Ok(PasswordResetRequest::Sent { token })
    }

    pub async fn reset(
        &self,
        credentials: PasswordResetCredentials<'_>,
        now: i64,
    ) -> Result<bool, PasswordResetError<R::Error>> {
        let Some(stored) = self.repository.token(credentials.email).await.map_err(PasswordResetError::Repository)?
        else {
            return Ok(false);
        };
        let digest = digest_token(credentials.token);
        if stored.digest != digest || stored.created_at < now - self.expires_after_seconds {
            return Ok(false);
        }
        let password_hash = self.passwords.hash(credentials.password).await?;
        self.repository
            .consume_token(ConsumedPasswordReset {
                email: credentials.email,
                expected_digest: &digest,
                password_hash: &password_hash,
                updated_at: now,
            })
            .await
            .map_err(PasswordResetError::Repository)
    }
}

#[derive(Debug, Error)]
pub enum PasswordResetError<E>
where
    E: Error + 'static,
{
    #[error("password reset repository operation failed")]
    Repository(#[source] E),
    #[error(transparent)]
    Password(#[from] PasswordHashError),
}
