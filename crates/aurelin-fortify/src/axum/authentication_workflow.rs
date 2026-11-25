use std::{net::IpAddr, time::Duration};

use aurelin_framework::{auth::RateLimitOutcome, timebox::Timebox};
use axum_login::AuthUser;
use validator::ValidateEmail;

use crate::LoginRateLimiter;

use super::{
    CreateUserOutcome, Credentials, FortifyBackend, FortifyHttpError,
    events::{self, Failed, Lockout, Login, Logout, Registered},
    login_session::set_login_expiry,
};

#[cfg(feature = "two-factor-authentication")]
use super::{FortifyUser, login_session::PENDING_REMEMBER_LOGIN};

#[derive(Clone, Debug)]
pub struct LoginAttempt {
    pub email: String,
    pub password: String,
    pub remember: bool,
    pub ip_address: IpAddr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginOutcome {
    AlreadyAuthenticated,
    Authenticated,
    #[cfg(feature = "two-factor-authentication")]
    TwoFactorChallenge,
    InvalidCredentials {
        remaining: u32,
    },
    RateLimited {
        retry_after: Duration,
    },
}

#[derive(Clone, Debug)]
pub struct RegistrationAttempt {
    pub name: String,
    pub email: String,
    pub password: String,
    pub password_confirmation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationOutcome {
    AlreadyAuthenticated,
    Created,
    EmailAlreadyExists,
    InvalidInput { field: &'static str, message: String },
}

pub async fn attempt_login<B: FortifyBackend>(
    auth: &mut axum_login::AuthSession<B>,
    attempt: LoginAttempt,
) -> Result<LoginOutcome, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(LoginOutcome::AlreadyAuthenticated);
    }
    let key = LoginRateLimiter::key(&attempt.email, attempt.ip_address);
    if let Some(retry_after) = auth.backend.login_retry_after(&key).await {
        events::observe(auth.backend.authentication_events(), Lockout).await;
        return Ok(LoginOutcome::RateLimited { retry_after });
    }
    let timebox = Timebox::start(auth.backend.authentication_timebox_duration());
    let user = auth
        .authenticate(Credentials { email: attempt.email, password: attempt.password })
        .await
        .map_err(FortifyHttpError::auth_session)?;
    let Some(user) = user else {
        timebox.wait().await;
        tracing::warn!("authentication failed");
        let remaining = match auth.backend.record_failed_login(&key).await {
            RateLimitOutcome::Allowed { remaining } => remaining,
            RateLimitOutcome::Limited { .. } => 0,
        };
        events::observe(auth.backend.authentication_events(), Failed).await;
        return Ok(LoginOutcome::InvalidCredentials { remaining });
    };
    #[cfg(feature = "two-factor-authentication")]
    if user.two_factor_is_enabled() {
        auth.session.insert("login.id", user.id()).await?;
        auth.session.insert(PENDING_REMEMBER_LOGIN, attempt.remember).await?;
        return Ok(LoginOutcome::TwoFactorChallenge);
    }
    auth.backend.clear_failed_logins(&key).await;
    auth.login(&user).await.map_err(FortifyHttpError::auth_session)?;
    set_login_expiry(&auth.session, attempt.remember);
    events::observe(auth.backend.authentication_events(), Login { user_id: user.id(), remember: attempt.remember })
        .await;
    Ok(LoginOutcome::Authenticated)
}

pub async fn attempt_registration<B: FortifyBackend>(
    auth: &mut axum_login::AuthSession<B>,
    attempt: RegistrationAttempt,
) -> Result<RegistrationOutcome, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(RegistrationOutcome::AlreadyAuthenticated);
    }
    let name_length = attempt.name.chars().count();
    if !(1..=255).contains(&name_length) {
        return Ok(RegistrationOutcome::InvalidInput {
            field: "name",
            message: "The name must be between 1 and 255 characters.".into(),
        });
    }
    if attempt.email.chars().count() > 255 || !attempt.email.validate_email() {
        return Ok(RegistrationOutcome::InvalidInput {
            field: "email",
            message: "The email must be a valid email address.".into(),
        });
    }
    if attempt.password.chars().count() < 8 {
        return Ok(RegistrationOutcome::InvalidInput {
            field: "password",
            message: "The password must be at least 8 characters.".into(),
        });
    }
    if attempt.password != attempt.password_confirmation {
        return Ok(RegistrationOutcome::InvalidInput {
            field: "password",
            message: "The password confirmation does not match.".into(),
        });
    }
    if let Err(message) = auth.backend.validate_password(&attempt.password).await {
        return Ok(RegistrationOutcome::InvalidInput { field: "password", message });
    }
    let user = match auth
        .backend
        .create_user(&attempt.name, &attempt.email, &attempt.password)
        .await
        .map_err(FortifyHttpError::backend)?
    {
        CreateUserOutcome::Created(user) => user,
        CreateUserOutcome::EmailAlreadyExists => {
            return Ok(RegistrationOutcome::EmailAlreadyExists);
        }
    };
    events::observe(auth.backend.authentication_events(), Registered { user_id: user.id() }).await;
    auth.backend.send_email_verification(&user).await.map_err(FortifyHttpError::backend)?;
    auth.login(&user).await.map_err(FortifyHttpError::auth_session)?;
    events::observe(auth.backend.authentication_events(), Login { user_id: user.id(), remember: false }).await;
    tracing::info!(user_id = user.id(), "user registered");
    Ok(RegistrationOutcome::Created)
}

pub async fn logout<B: FortifyBackend>(auth: &mut axum_login::AuthSession<B>) -> Result<bool, FortifyHttpError> {
    let Some(user_id) = auth.user.as_ref().map(|user| user.id()) else {
        return Ok(false);
    };
    auth.logout().await.map_err(FortifyHttpError::auth_session)?;
    events::observe(auth.backend.authentication_events(), Logout { user_id }).await;
    Ok(true)
}
