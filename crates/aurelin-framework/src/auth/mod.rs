mod password;
mod password_reset;
mod rate_limit;

pub use password::{BcryptPasswordHasher, PasswordHashError};
pub use password_reset::{
    ConsumedPasswordReset, PasswordResetBroker, PasswordResetCredentials, PasswordResetError, PasswordResetRepository,
    PasswordResetRequest, StoredPasswordResetToken,
};
pub use rate_limit::FixedWindowRateLimiter;

pub fn digest_token(token: &str) -> String {
    use sha2::{Digest, Sha256};

    format!("{:x}", Sha256::digest(token.as_bytes()))
}
