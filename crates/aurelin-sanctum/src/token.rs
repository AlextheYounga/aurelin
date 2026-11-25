use std::{error::Error, fmt, future::Future};

use aurelin_framework::auth::digest_token;
use rand::{Rng, distr::Alphanumeric};
use thiserror::Error;

use crate::Abilities;

/// A newly generated secret. Persist only its digest, never this value.
pub struct NewToken(String);

impl NewToken {
    pub fn generate() -> Self {
        Self(rand::rng().sample_iter(Alphanumeric).take(64).map(char::from).collect())
    }

    pub fn digest(&self) -> String {
        digest_token(&self.0)
    }

    /// Consume the secret after persistence and return the credential once.
    pub fn into_plaintext(self, id: u64) -> String {
        format!("{id}|{}", self.0)
    }
}

impl fmt::Debug for NewToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NewToken([REDACTED])")
    }
}

/// A strictly parsed credential. Its secret is never included in debug output.
pub struct ParsedToken<'a> {
    id: u64,
    secret: &'a str,
}

impl<'a> ParsedToken<'a> {
    pub fn parse(value: &'a str) -> Result<Self, TokenError> {
        let (id, secret) = value.split_once('|').ok_or(TokenError::Malformed)?;
        if id.is_empty()
            || !id.bytes().all(|byte| byte.is_ascii_digit())
            || secret.len() != 64
            || !secret.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(TokenError::Malformed);
        }
        let id = id.parse().map_err(|_| TokenError::Malformed)?;
        Ok(Self { id, secret })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn matches_digest(&self, digest: &str) -> bool {
        constant_time_eq::constant_time_eq(digest_token(self.secret).as_bytes(), digest.as_bytes())
    }
}

impl fmt::Debug for ParsedToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParsedToken").field("id", &self.id).field("secret", &"[REDACTED]").finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("malformed personal access token")]
    Malformed,
}

/// Values to persist when issuing a token. The digest is sensitive and must not be logged.
pub struct TokenCreation<'a, Owner> {
    pub owner: &'a Owner,
    pub name: &'a str,
    pub digest: &'a str,
    pub abilities: &'a Abilities,
    pub expires_at: Option<i64>,
}

/// Application persistence boundary. Implementations must scope management to
/// the owner and atomically refuse a last-used update after revocation or
/// expiration, including when the record changes between lookup and update.
pub trait TokenRepository: Send + Sync {
    type Owner: Send + Sync;
    type Error: Error + Send + Sync + 'static;

    fn create(&self, token: TokenCreation<'_, Self::Owner>) -> impl Future<Output = Result<u64, Self::Error>> + Send;
    fn find(&self, id: u64) -> impl Future<Output = Result<Option<StoredToken<Self::Owner>>, Self::Error>> + Send;
    fn mark_used(&self, id: u64, digest: &str, now: i64) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn revoke(&self, owner: &Self::Owner, id: u64) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn list(&self, owner: &Self::Owner) -> impl Future<Output = Result<Vec<TokenMetadata>, Self::Error>> + Send;
}

/// Public metadata deliberately excludes the token digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenMetadata {
    pub id: u64,
    pub name: String,
    pub abilities: Abilities,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
}

/// Repository result; intentionally has no Debug implementation because it
/// contains a credential digest.
pub struct StoredToken<Owner> {
    pub owner: Owner,
    pub digest: String,
    pub abilities: Abilities,
    pub expires_at: Option<i64>,
}

impl<Owner> StoredToken<Owner> {
    /// Expiry timestamps are Unix seconds; a token expires at the deadline.
    pub fn is_expired(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|expires_at| now >= expires_at)
    }
}
