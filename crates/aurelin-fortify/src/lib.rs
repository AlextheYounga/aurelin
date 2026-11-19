//! Headless authentication workflows translated from Laravel Fortify.
//!
//! Applications provide user persistence and starter views through the action
//! contracts in this crate. Framework primitives remain in `aurelin-framework`.

pub use aurelin_passkeys as passkeys;

mod actions;
mod email_verification;
mod email_verification_rate_limiter;
mod login_rate_limiter;
mod password_reset;
mod two_factor_authentication;
mod two_factor_rate_limiter;

pub use actions::{CreatesNewUsers, RegistrationInput, ResetPasswordInput, ResetsUserPasswords};
pub use email_verification::{EmailVerification, EmailVerificationError, EmailVerificationRequest};
pub use email_verification_rate_limiter::{EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE, EmailVerificationRateLimiter};
pub use login_rate_limiter::{LOGIN_ATTEMPTS_PER_MINUTE, LoginRateLimiter};
pub use password_reset::{ResetPasswords, ResetPasswordsError};
pub use two_factor_authentication::{
    TwoFactorAuthentication, TwoFactorChallenge, TwoFactorChallengeOutcome, TwoFactorError, TwoFactorSetup,
    TwoFactorState,
};
pub use two_factor_rate_limiter::{TWO_FACTOR_ATTEMPTS_PER_MINUTE, TwoFactorRateLimiter};
