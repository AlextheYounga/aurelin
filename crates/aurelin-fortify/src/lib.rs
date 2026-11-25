//! Headless authentication workflows translated from Laravel Fortify.
//!
//! Applications provide user persistence and starter views through the action
//! contracts in this crate. Framework primitives remain in `aurelin-framework`.
//!
//! No features are enabled by default. Enable `two-factor-authentication` for
//! TOTP, recovery codes and their routes, or `passkeys` for the independent
//! `aurelin-passkeys` package and its Fortify routes. Feature-specific backend
//! methods and frontend fields exist only when that capability is enabled.

#[cfg(feature = "passkeys")]
pub use aurelin_passkeys as passkeys;

mod actions;
pub mod axum;
mod email_verification;
mod email_verification_rate_limiter;
mod login_rate_limiter;
#[cfg(feature = "passkeys")]
mod passkey_rate_limiter;
mod password_reset;
#[cfg(feature = "two-factor-authentication")]
mod two_factor_authentication;
#[cfg(feature = "two-factor-authentication")]
mod two_factor_rate_limiter;

pub use actions::{CreatesNewUsers, RegistrationInput, ResetPasswordInput, ResetsUserPasswords};
pub use email_verification::{EmailVerification, EmailVerificationError, EmailVerificationRequest};
pub use email_verification_rate_limiter::{EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE, EmailVerificationRateLimiter};
pub use login_rate_limiter::{LOGIN_ATTEMPTS_PER_MINUTE, LoginRateLimiter};
#[cfg(feature = "passkeys")]
pub use passkey_rate_limiter::{PASSKEY_ATTEMPTS_PER_MINUTE, PasskeyRateLimiter};
pub use password_reset::{ResetPasswords, ResetPasswordsError};
#[cfg(feature = "two-factor-authentication")]
pub use two_factor_authentication::{
    TwoFactorAuthentication, TwoFactorChallenge, TwoFactorChallengeOutcome, TwoFactorError, TwoFactorSetup,
    TwoFactorState,
};
#[cfg(feature = "two-factor-authentication")]
pub use two_factor_rate_limiter::{TWO_FACTOR_ATTEMPTS_PER_MINUTE, TwoFactorRateLimiter};
