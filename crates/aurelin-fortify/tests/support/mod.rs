//! Controlled user persistence for the core-only Fortify HTTP test. Password
//! hashing and database behavior are tested by their owning framework modules.
use std::{convert::Infallible, time::Duration};

use aurelin_fortify::axum::{
    CreateUserOutcome, Credentials, EmailVerificationRequest, FortifyBackend, FortifyComponents, FortifyFrontend,
    FortifyUser,
};
use aurelin_framework::{
    auth::{PasswordResetRequest, RateLimitOutcome},
    axum::response::{Html, IntoResponse, Response},
    inertia::axum::HtmlResponseContext,
};
use axum_login::{AuthUser, AuthnBackend, UserId};

#[derive(Clone, Debug)]
pub struct User;

impl AuthUser for User {
    type Id = i64;
    fn id(&self) -> i64 {
        1
    }
    fn session_auth_hash(&self) -> &[u8] {
        b"test-session-hash"
    }
}

impl FortifyUser for User {
    type Public = &'static str;
    fn email(&self) -> &str {
        "ada@example.com"
    }
    fn email_is_verified(&self) -> bool {
        true
    }
    fn public(&self) -> Self::Public {
        "ada@example.com"
    }
}

#[derive(Clone)]
pub struct Backend;

impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = Infallible;

    async fn authenticate(&self, credentials: Credentials) -> Result<Option<User>, Infallible> {
        Ok((credentials.email == "ada@example.com" && credentials.password == "test-password").then_some(User))
    }
    async fn get_user(&self, id: &UserId<Self>) -> Result<Option<User>, Infallible> {
        Ok((*id == 1).then_some(User))
    }
}

impl FortifyBackend for Backend {
    fn authentication_timebox_duration(&self) -> Duration {
        Duration::ZERO
    }
    fn password_rules(&self) -> &'static str {
        "min:8"
    }
    fn application_url(&self) -> &str {
        "http://localhost"
    }
    async fn validate_password(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    async fn login_retry_after(&self, _: &str) -> Option<Duration> {
        None
    }
    async fn record_failed_login(&self, _: &str) -> RateLimitOutcome {
        RateLimitOutcome::Allowed { remaining: 4 }
    }
    async fn clear_failed_logins(&self, _: &str) {}
    async fn create_user(&self, _: &str, _: &str, _: &str) -> Result<CreateUserOutcome<User>, Infallible> {
        Ok(CreateUserOutcome::Created(User))
    }
    async fn send_email_verification(&self, _: &User) -> Result<(), Infallible> {
        Ok(())
    }
    async fn create_password_reset_token(&self, _: &str) -> Result<PasswordResetRequest, Infallible> {
        Ok(PasswordResetRequest::UnknownUser)
    }
    async fn send_password_reset(&self, _: &str, _: &str) -> Result<(), Infallible> {
        Ok(())
    }
    async fn reset_password(&self, _: &str, _: &str, _: &str) -> Result<bool, Infallible> {
        Ok(false)
    }
    async fn email_verification_attempt(&self, _: i64) -> RateLimitOutcome {
        RateLimitOutcome::Allowed { remaining: 5 }
    }
    async fn verify_email(&self, _: EmailVerificationRequest<'_, User>) -> Result<bool, Infallible> {
        Ok(false)
    }
}

#[derive(Clone)]
pub struct Frontend;

impl FortifyFrontend for Frontend {
    fn fortify_components(&self) -> FortifyComponents {
        FortifyComponents {
            dashboard: "Dashboard",
            login: "Login",
            register: "Register",
            forgot_password: "ForgotPassword",
            reset_password: "ResetPassword",
            confirm_password: "ConfirmPassword",
            verify_email: "VerifyEmail",
        }
    }
    fn fortify_document(&self, context: HtmlResponseContext) -> Response {
        Html(context.data_page().to_owned()).into_response()
    }
}
