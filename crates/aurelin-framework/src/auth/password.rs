use std::str::FromStr;

use bcrypt::{DEFAULT_COST, HashParts, hash, verify};
use thiserror::Error;
use tokio::task::{JoinError, spawn_blocking};

#[derive(Clone, Copy, Debug)]
pub struct BcryptPasswordHasher {
    cost: u32,
}

impl Default for BcryptPasswordHasher {
    fn default() -> Self {
        Self { cost: DEFAULT_COST }
    }
}

impl BcryptPasswordHasher {
    pub fn new(cost: u32) -> Self {
        Self { cost }
    }

    pub async fn hash(&self, password: &str) -> Result<String, PasswordHashError> {
        let password = password.to_owned();
        let cost = self.cost;
        spawn_blocking(move || hash(password, cost).map_err(|_| PasswordHashError::Password)).await?
    }

    pub async fn verify(&self, password: &str, password_hash: &str) -> Result<bool, PasswordHashError> {
        let password = password.to_owned();
        let password_hash = password_hash.to_owned();
        spawn_blocking(move || verify(password, &password_hash).map_err(|_| PasswordHashError::Password)).await?
    }

    pub fn needs_rehash(&self, password_hash: &str) -> Result<bool, PasswordHashError> {
        let parts = HashParts::from_str(password_hash).map_err(|_| PasswordHashError::Password)?;
        Ok(parts.get_cost() != self.cost)
    }
}

#[derive(Debug, Error)]
pub enum PasswordHashError {
    #[error("password operation failed")]
    Password,
    #[error("password worker failed")]
    Worker(#[from] JoinError),
}
