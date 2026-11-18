//! Headless authentication workflows translated from Laravel Fortify.
//!
//! Applications provide user persistence and starter views through the action
//! contracts in this crate. Framework primitives remain in `aurelin-framework`.

mod actions;
mod email_verification;
mod login_rate_limiter;

pub use actions::{CreatesNewUsers, RegistrationInput, ResetPasswordInput, ResetsUserPasswords};
pub use email_verification::{EmailVerification, EmailVerificationError, EmailVerificationRequest};
pub use login_rate_limiter::LoginRateLimiter;
