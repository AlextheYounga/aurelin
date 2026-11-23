use std::{
    error::Error,
    future::Future,
    time::{Duration, SystemTimeError},
};

use aurelin_framework::{
    auth::{PasswordResetRequest, RateLimitOutcome},
    axum::{
        Router,
        http::StatusCode,
        response::{Html, IntoResponse, Response},
    },
    inertia::axum::{HtmlResponseContext, InertiaError},
};
use axum_login::{AuthUser, AuthnBackend};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use tower_sessions::session;

use crate::passkeys::PasskeyCeremony;

pub const DEFAULT_AUTHENTICATION_TIMEBOX_DURATION: Duration = Duration::from_millis(200);

mod authentication;
mod authentication_workflow;
mod email_verification;
mod feedback;
mod login_session;
mod passkeys;
mod password_confirmation;
mod password_reset;
mod two_factor_authentication;

pub use authentication_workflow::{
    LoginAttempt, LoginOutcome, RegistrationAttempt, RegistrationOutcome, attempt_login, attempt_registration, logout,
};
pub use password_confirmation::{
    DEFAULT_PASSWORD_TIMEOUT_SECONDS, PASSWORD_CONFIRMATION_REDIRECT, PASSWORD_CONFIRMED_AT, password_is_confirmed,
};

#[derive(Clone, Copy, Debug)]
pub struct FortifyComponents {
    pub dashboard: &'static str,
    pub login: &'static str,
    pub register: &'static str,
    pub forgot_password: &'static str,
    pub reset_password: &'static str,
    pub confirm_password: &'static str,
    pub verify_email: &'static str,
    pub two_factor_challenge: &'static str,
}

pub trait FortifyFrontend: Clone + Send + Sync + 'static {
    fn fortify_components(&self) -> FortifyComponents;
    fn fortify_document(&self, context: HtmlResponseContext) -> Html<String>;
}

pub trait FortifyUser: AuthUser<Id = i64> + Clone + Send + Sync + 'static {
    type Public: Serialize + Send;

    fn email(&self) -> &str;
    fn email_is_verified(&self) -> bool;
    fn two_factor_is_enabled(&self) -> bool;
    fn public(&self) -> Self::Public;
}

#[derive(Clone, Debug)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

pub enum CreateUserOutcome<U> {
    Created(U),
    EmailAlreadyExists,
}

pub enum PasskeyRegistrationOutcome {
    Registered { id: String, name: String },
    AlreadyRegistered,
    Invalid,
}

pub enum PasskeyLoginOutcome<U> {
    Authenticated(U),
    Invalid,
}

pub struct EmailVerificationRequest<'a, U> {
    pub user: &'a U,
    pub email_hash: &'a str,
    pub expires: i64,
    pub signature: &'a str,
}

pub struct PasskeyRegistrationRequest<'a, U> {
    pub ceremony_id: &'a str,
    pub user: &'a U,
    pub name: String,
    pub credential: Value,
}

pub trait FortifyBackend:
    AuthnBackend<User: FortifyUser, Credentials = Credentials> + Clone + Send + Sync + 'static
where
    Self::Error: Error + Send + Sync + 'static,
{
    fn authentication_timebox_duration(&self) -> Duration {
        DEFAULT_AUTHENTICATION_TIMEBOX_DURATION
    }

    fn password_rules(&self) -> &'static str;
    fn application_url(&self) -> &str;
    fn validate_password(&self, password: &str) -> impl Future<Output = Result<(), String>> + Send;
    fn login_retry_after(&self, key: &str) -> impl Future<Output = Option<Duration>> + Send;
    fn record_failed_login(&self, key: &str) -> impl Future<Output = RateLimitOutcome> + Send;
    fn clear_failed_logins(&self, key: &str) -> impl Future<Output = ()> + Send;
    fn create_user(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> impl Future<Output = Result<CreateUserOutcome<Self::User>, Self::Error>> + Send;
    fn send_email_verification(&self, user: &Self::User) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn create_password_reset_token(
        &self,
        email: &str,
    ) -> impl Future<Output = Result<PasswordResetRequest, Self::Error>> + Send;
    fn send_password_reset(&self, email: &str, token: &str) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn reset_password(
        &self,
        email: &str,
        token: &str,
        password: &str,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn email_verification_attempt(&self, user_id: i64) -> impl Future<Output = RateLimitOutcome> + Send;
    fn verify_email(
        &self,
        request: EmailVerificationRequest<'_, Self::User>,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn two_factor_attempt(&self, user_id: i64) -> impl Future<Output = RateLimitOutcome> + Send;
    fn challenged_user(&self, user_id: i64) -> impl Future<Output = Result<Option<Self::User>, Self::Error>> + Send;
    fn challenge_two_factor(
        &self,
        user: &Self::User,
        code: Option<&str>,
        recovery_code: Option<&str>,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn enable_two_factor(&self, user: &Self::User) -> impl Future<Output = Result<Self::User, Self::Error>> + Send;
    fn confirm_two_factor(
        &self,
        user: &Self::User,
        code: &str,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn disable_two_factor(&self, user: &Self::User) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn two_factor_qr_code(&self, user: &Self::User) -> Result<Option<String>, Self::Error>;
    fn two_factor_secret_key(&self, user: &Self::User) -> Result<Option<String>, Self::Error>;
    fn two_factor_recovery_codes(&self, user: &Self::User) -> Result<Vec<String>, Self::Error>;
    fn regenerate_two_factor_recovery_codes(
        &self,
        user: &Self::User,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn passkey_attempt(&self, key: &str) -> impl Future<Output = RateLimitOutcome> + Send;
    fn start_passkey_registration(
        &self,
        user: &Self::User,
    ) -> impl Future<Output = Result<PasskeyCeremony, Self::Error>> + Send;
    fn finish_passkey_registration(
        &self,
        request: PasskeyRegistrationRequest<'_, Self::User>,
    ) -> impl Future<Output = Result<PasskeyRegistrationOutcome, Self::Error>> + Send;
    fn start_passkey_login(&self) -> Result<PasskeyCeremony, Self::Error>;
    fn finish_passkey_login(
        &self,
        ceremony_id: &str,
        credential: Value,
    ) -> impl Future<Output = Result<PasskeyLoginOutcome<Self::User>, Self::Error>> + Send;
    fn start_passkey_confirmation(
        &self,
        user: &Self::User,
    ) -> impl Future<Output = Result<PasskeyCeremony, Self::Error>> + Send;
    fn finish_passkey_confirmation(
        &self,
        ceremony_id: &str,
        user: &Self::User,
        credential: Value,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn delete_passkey(&self, user_id: i64, passkey_id: i64) -> impl Future<Output = Result<bool, Self::Error>> + Send;
}

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
    B::Error: Error + Send + Sync + 'static,
{
    Router::new()
        .merge(authentication::routes::<F, B>())
        .merge(password_reset::routes::<F, B>())
        .merge(email_verification::routes::<F, B>())
        .merge(password_confirmation::routes::<F, B>())
        .merge(two_factor_authentication::routes::<F, B>())
        .merge(passkeys::routes::<F, B>())
}

#[derive(Debug, Error)]
pub enum FortifyHttpError {
    #[error("authentication backend failed: {0}")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
    #[error("authentication session failed: {0}")]
    AuthSession(#[source] Box<dyn Error + Send + Sync>),
    #[error("CSRF session operation failed: {0}")]
    Csrf(String),
    #[error(transparent)]
    Inertia(#[from] InertiaError),
    #[error(transparent)]
    Session(#[from] session::Error),
    #[error("system clock is before the Unix epoch")]
    Clock(#[from] SystemTimeError),
}

impl FortifyHttpError {
    fn backend(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Backend(Box::new(error))
    }

    fn auth_session(error: impl Error + Send + Sync + 'static) -> Self {
        Self::AuthSession(Box::new(error))
    }
}

impl IntoResponse for FortifyHttpError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self, "Fortify request failed");
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    }
}

pub(crate) type AuthSession<B> = axum_login::AuthSession<B>;
