use crate::TWO_FACTOR_ATTEMPTS_PER_MINUTE;
use aurelin_framework::http as rate_limit;
use aurelin_framework::{
    auth::RateLimitOutcome,
    axum::{
        Json, Router,
        extract::{State, rejection::JsonRejection},
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_login::AuthUser;
use axum_tower_sessions_csrf::get_or_create_token;
use serde::{Deserialize, Serialize};

use super::{
    AuthSession, FortifyBackend, FortifyFrontend, FortifyHttpError,
    events::{self, Login},
    feedback::{FLASH_ERRORS, ValidationErrors, redirect_with_error},
    login_session::{PENDING_REMEMBER_LOGIN, set_login_expiry},
    password_confirmation::{PASSWORD_CONFIRMATION_REDIRECT, password_is_confirmed},
};

const CHALLENGED_USER_ID: &str = "login.id";

#[derive(Serialize)]
struct ChallengeProps {
    errors: ValidationErrors,
    csrf_token: String,
}

#[derive(Deserialize)]
struct ChallengeInput {
    code: Option<String>,
    recovery_code: Option<String>,
}

#[derive(Deserialize)]
struct ConfirmationInput {
    code: String,
}

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
{
    Router::new()
        .route("/two-factor-challenge", get(challenge::<F, B>).post(store_challenge::<B>))
        .route("/user/two-factor-authentication", post(enable::<B>).delete(disable::<B>))
        .route("/user/confirmed-two-factor-authentication", post(confirm::<B>))
        .route("/user/two-factor-qr-code", get(qr_code::<B>))
        .route("/user/two-factor-secret-key", get(secret_key::<B>))
        .route("/user/two-factor-recovery-codes", get(recovery_codes::<B>).post(regenerate_recovery_codes::<B>))
}

async fn challenge<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() || auth.session.get::<i64>(CHALLENGED_USER_ID).await?.is_none() {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    let errors = auth.session.remove::<ValidationErrors>(FLASH_ERRORS).await?.unwrap_or_default();
    Ok(request.render(
        Inertia::response(frontend.fortify_components().two_factor_challenge, ChallengeProps { errors, csrf_token }),
        move |context| frontend.fortify_document(context),
    )?)
}

async fn store_challenge<B: FortifyBackend>(
    mut auth: AuthSession<B>,
    request: InertiaRequest,
    input: Result<Json<ChallengeInput>, JsonRejection>,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let Some(user_id) = auth.session.get::<i64>(CHALLENGED_USER_ID).await? else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    let remaining = match auth.backend.two_factor_attempt(user_id).await {
        RateLimitOutcome::Allowed { remaining } => remaining,
        RateLimitOutcome::Limited { retry_after } => {
            return Ok(rate_limit::too_many_attempts(TWO_FACTOR_ATTEMPTS_PER_MINUTE, retry_after));
        }
    };
    let Some(user) = auth.backend.challenged_user(user_id).await.map_err(FortifyHttpError::backend)? else {
        auth.session.remove::<i64>(CHALLENGED_USER_ID).await?;
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !auth
        .backend
        .challenge_two_factor(&user, input.code.as_deref(), input.recovery_code.as_deref())
        .await
        .map_err(FortifyHttpError::backend)?
    {
        let mut response = redirect_with_error(
            &auth,
            &request,
            ("code", "The provided two factor authentication code was invalid."),
            "/two-factor-challenge",
        )
        .await?;
        rate_limit::add_rate_limit_headers(&mut response, TWO_FACTOR_ATTEMPTS_PER_MINUTE, remaining, None);
        return Ok(response);
    }
    auth.session.remove::<i64>(CHALLENGED_USER_ID).await?;
    auth.login(&user).await.map_err(FortifyHttpError::auth_session)?;
    let remember = auth.session.remove::<bool>(PENDING_REMEMBER_LOGIN).await?.unwrap_or_default();
    set_login_expiry(&auth.session, remember);
    events::observe(auth.backend.authentication_events(), Login { user_id: user.id(), remember }).await;
    let mut response = request.redirect(Inertia::redirect("/dashboard"))?;
    rate_limit::add_rate_limit_headers(&mut response, TWO_FACTOR_ATTEMPTS_PER_MINUTE, remaining, None);
    Ok(response)
}

async fn enable<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    auth.backend.enable_two_factor(user).await.map_err(FortifyHttpError::backend)?;
    Ok(request.redirect(Inertia::redirect("/settings/security"))?)
}

async fn confirm<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
    input: Result<Json<ConfirmationInput>, JsonRejection>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    if !auth.backend.confirm_two_factor(user, &input.code).await.map_err(FortifyHttpError::backend)? {
        return redirect_with_error(
            &auth,
            &request,
            ("code", "The provided two factor authentication code was invalid."),
            "/settings/security",
        )
        .await;
    }
    Ok(request.redirect(Inertia::redirect("/settings/security"))?)
}

async fn disable<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    auth.backend.disable_two_factor(user).await.map_err(FortifyHttpError::backend)?;
    Ok(request.redirect(Inertia::redirect("/settings/security"))?)
}

async fn qr_code<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    Ok(Json(serde_json::json!({
        "svg": auth.backend.two_factor_qr_code(user).map_err(FortifyHttpError::backend)?.map(|png| format!(
            "<img src=\"data:image/png;base64,{png}\" alt=\"Two-factor authentication QR code\">"
        ))
    }))
    .into_response())
}

async fn secret_key<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    Ok(Json(serde_json::json!({
        "secretKey": auth.backend.two_factor_secret_key(user).map_err(FortifyHttpError::backend)?
    }))
    .into_response())
}

async fn recovery_codes<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    Ok(Json(auth.backend.two_factor_recovery_codes(user).map_err(FortifyHttpError::backend)?).into_response())
}

async fn regenerate_recovery_codes<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    auth.backend.regenerate_two_factor_recovery_codes(user).await.map_err(FortifyHttpError::backend)?;
    Ok(request.redirect(Inertia::redirect("/settings/security"))?)
}

async fn confirmed_user<'a, B: FortifyBackend>(
    auth: &'a AuthSession<B>,
    _request: &InertiaRequest,
) -> Result<Option<&'a B::User>, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(None);
    };
    if !password_is_confirmed(auth).await? {
        auth.session.insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/security").await?;
        return Ok(None);
    }
    Ok(Some(user))
}
