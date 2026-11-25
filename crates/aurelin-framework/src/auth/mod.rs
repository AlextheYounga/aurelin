mod compromised_password;
mod encryption;
mod password;
mod password_reset;
mod rate_limit;

pub use password::{BcryptPasswordHasher, PasswordHashError};
pub use password_reset::{
    PasswordResetBroker, PasswordResetCredentials, PasswordResetError, PasswordResetRepository, PasswordResetRequest,
    StoredPasswordResetToken, ValidatedPasswordReset,
};
pub use rate_limit::{FixedWindowRateLimiter, RateLimitOutcome};

pub fn digest_token(token: &str) -> String {
    use sha2::{Digest, Sha256};

    Sha256::digest(token.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}
pub use compromised_password::CompromisedPasswordChecker;
pub use encryption::{ApplicationEncrypter, EncryptionError};
