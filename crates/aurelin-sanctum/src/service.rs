use std::{error::Error, fmt};

use thiserror::Error;

use crate::{Abilities, NewToken, ParsedToken, TokenCreation, TokenMetadata, TokenRepository};

pub struct IssueToken<'a, Owner> {
    pub owner: &'a Owner,
    pub name: &'a str,
    pub abilities: &'a Abilities,
    pub expires_at: Option<i64>,
    /// Set only after the application has authorized unrestricted access.
    pub allow_wildcard: bool,
}

pub struct IssuedToken {
    pub id: u64,
    plaintext: String,
}

impl IssuedToken {
    pub fn into_plaintext(self) -> String {
        self.plaintext
    }
}

impl fmt::Debug for IssuedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuedToken").field("id", &self.id).field("plaintext", &"[REDACTED]").finish()
    }
}

#[derive(Clone, Debug)]
pub struct AuthenticatedToken<Owner> {
    pub owner: Owner,
    pub id: u64,
    pub abilities: Abilities,
}

#[derive(Error)]
pub enum ServiceError<E: Error + 'static> {
    #[error("authentication failed")]
    Unauthenticated,
    #[error("unrestricted token ability was not authorized")]
    WildcardNotAllowed,
    #[error("token repository failed")]
    Repository(E),
}

impl<E: Error + 'static> fmt::Debug for ServiceError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthenticated => f.write_str("Unauthenticated"),
            Self::WildcardNotAllowed => f.write_str("WildcardNotAllowed"),
            Self::Repository(_) => f.write_str("Repository([REDACTED])"),
        }
    }
}

#[derive(Clone)]
pub struct TokenService<R> {
    repository: R,
}

impl<R: TokenRepository> TokenService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn issue(&self, input: IssueToken<'_, R::Owner>) -> Result<IssuedToken, ServiceError<R::Error>> {
        if input.abilities.as_slice().iter().any(|ability| ability == "*") && !input.allow_wildcard {
            return Err(ServiceError::WildcardNotAllowed);
        }
        let token = NewToken::generate();
        let digest = token.digest();
        let id = self
            .repository
            .create(TokenCreation {
                owner: input.owner,
                name: input.name,
                digest: &digest,
                abilities: input.abilities,
                expires_at: input.expires_at,
            })
            .await
            .map_err(ServiceError::Repository)?;
        Ok(IssuedToken { id, plaintext: token.into_plaintext(id) })
    }

    pub async fn authenticate(
        &self,
        plaintext: &str,
        now: i64,
    ) -> Result<AuthenticatedToken<R::Owner>, ServiceError<R::Error>> {
        let parsed = ParsedToken::parse(plaintext).map_err(|_| ServiceError::Unauthenticated)?;
        let token = self
            .repository
            .find(parsed.id())
            .await
            .map_err(ServiceError::Repository)?
            .ok_or(ServiceError::Unauthenticated)?;
        if token.is_expired(now) || !parsed.matches_digest(&token.digest) {
            return Err(ServiceError::Unauthenticated);
        }
        if !self.repository.mark_used(parsed.id(), &token.digest, now).await.map_err(ServiceError::Repository)? {
            return Err(ServiceError::Unauthenticated);
        }
        Ok(AuthenticatedToken { owner: token.owner, id: parsed.id(), abilities: token.abilities })
    }

    pub async fn list(&self, owner: &R::Owner) -> Result<Vec<TokenMetadata>, ServiceError<R::Error>> {
        self.repository.list(owner).await.map_err(ServiceError::Repository)
    }

    pub async fn revoke(&self, owner: &R::Owner, id: u64) -> Result<bool, ServiceError<R::Error>> {
        self.repository.revoke(owner, id).await.map_err(ServiceError::Repository)
    }

    pub async fn revoke_current(&self, current: &AuthenticatedToken<R::Owner>) -> Result<bool, ServiceError<R::Error>> {
        self.revoke(&current.owner, current.id).await
    }
}
