use aurelin_fortify::TWO_FACTOR_ATTEMPTS_PER_MINUTE;
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
use axum_tower_sessions_csrf::get_or_create_token;
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthSession,
    frontend::Frontend,
    frontend_config::{TWO_FACTOR_CHALLENGE_COMPONENT, TWO_FACTOR_SETTINGS_COMPONENT},
    routes::{
        password_confirmation::{PASSWORD_CONFIRMATION_REDIRECT, password_is_confirmed},
        rate_limit,
        session_feedback::{FLASH_ERRORS, ValidationErrors, redirect_with_error},
        web_error::WebError,
    },
};

const CHALLENGED_USER_ID: &str = "login.id";

#[derive(Serialize)]
struct ChallengeProps {
    errors: ValidationErrors,
    csrf_token: String,
}

#[derive(Serialize)]
struct SettingsProps {
    enabled: bool,
    awaiting_confirmation: bool,
    secret_key: Option<String>,
    qr_code: Option<String>,
    recovery_codes: Vec<String>,
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

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route(
            "/two-factor-challenge",
            get(challenge).post(store_challenge),
        )
        .route("/settings/two-factor", get(settings))
        .route(
            "/user/two-factor-authentication",
            post(enable).delete(disable),
        )
        .route("/user/confirmed-two-factor-authentication", post(confirm))
        .route("/user/two-factor-qr-code", get(qr_code))
        .route("/user/two-factor-secret-key", get(secret_key))
        .route(
            "/user/two-factor-recovery-codes",
            get(recovery_codes).post(regenerate_recovery_codes),
        )
}

async fn challenge(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    if auth.user.is_some() || auth.session.get::<i64>(CHALLENGED_USER_ID).await?.is_none() {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let errors = auth
        .session
        .remove::<ValidationErrors>(FLASH_ERRORS)
        .await?
        .unwrap_or_default();
    Ok(request.render(
        Inertia::response(
            TWO_FACTOR_CHALLENGE_COMPONENT,
            ChallengeProps { errors, csrf_token },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn store_challenge(
    mut auth: AuthSession,
    request: InertiaRequest,
    input: Result<Json<ChallengeInput>, JsonRejection>,
) -> Result<Response, WebError> {
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
            return Ok(rate_limit::too_many_attempts(
                TWO_FACTOR_ATTEMPTS_PER_MINUTE,
                retry_after,
            ));
        }
    };
    let Some(user) = auth.backend.challenged_user(user_id).await? else {
        auth.session.remove::<i64>(CHALLENGED_USER_ID).await?;
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !auth
        .backend
        .challenge_two_factor(&user, input.code.as_deref(), input.recovery_code.as_deref())
        .await?
    {
        let mut response = redirect_with_error(
            &auth,
            &request,
            "code",
            "The provided two factor authentication code was invalid.",
            "/two-factor-challenge",
        )
        .await?;
        rate_limit::add_headers(
            &mut response,
            TWO_FACTOR_ATTEMPTS_PER_MINUTE,
            remaining,
            None,
        );
        return Ok(response);
    }
    auth.session.remove::<i64>(CHALLENGED_USER_ID).await?;
    auth.login(&user).await?;
    let mut response = request.redirect(Inertia::redirect("/dashboard"))?;
    rate_limit::add_headers(
        &mut response,
        TWO_FACTOR_ATTEMPTS_PER_MINUTE,
        remaining,
        None,
    );
    Ok(response)
}

async fn settings(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !password_is_confirmed(&auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/two-factor")
            .await?;
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let errors = auth
        .session
        .remove::<ValidationErrors>(FLASH_ERRORS)
        .await?
        .unwrap_or_default();
    let configured = user.two_factor_secret.is_some();
    let enabled = user.two_factor_confirmed_at.is_some();
    Ok(request.render(
        Inertia::response(
            TWO_FACTOR_SETTINGS_COMPONENT,
            SettingsProps {
                enabled,
                awaiting_confirmation: configured && !enabled,
                secret_key: auth.backend.two_factor_secret_key(user)?,
                qr_code: auth.backend.two_factor_qr_code(user)?,
                recovery_codes: if enabled {
                    auth.backend.two_factor_recovery_codes(user)?
                } else {
                    Vec::new()
                },
                errors,
                csrf_token,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn enable(auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    auth.backend.enable_two_factor(user).await?;
    Ok(request.redirect(Inertia::redirect("/settings/two-factor"))?)
}

async fn confirm(
    auth: AuthSession,
    request: InertiaRequest,
    input: Result<Json<ConfirmationInput>, JsonRejection>,
) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    if !auth.backend.confirm_two_factor(user, &input.code).await? {
        return redirect_with_error(
            &auth,
            &request,
            "code",
            "The provided two factor authentication code was invalid.",
            "/settings/two-factor",
        )
        .await;
    }
    Ok(request.redirect(Inertia::redirect("/settings/two-factor"))?)
}

async fn disable(auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    auth.backend.disable_two_factor(user).await?;
    Ok(request.redirect(Inertia::redirect("/settings/two-factor"))?)
}

async fn qr_code(auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    Ok(Json(serde_json::json!({
        "svg": auth.backend.two_factor_qr_code(user)?.map(|png| format!(
            "<img src=\"data:image/png;base64,{png}\" alt=\"Two-factor authentication QR code\">"
        ))
    }))
    .into_response())
}

async fn secret_key(auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    Ok(Json(serde_json::json!({
        "secretKey": auth.backend.two_factor_secret_key(user)?
    }))
    .into_response())
}

async fn recovery_codes(auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    Ok(Json(auth.backend.two_factor_recovery_codes(user)?).into_response())
}

async fn regenerate_recovery_codes(
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = confirmed_user(&auth, &request).await? else {
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    };
    auth.backend
        .regenerate_two_factor_recovery_codes(user)
        .await?;
    Ok(request.redirect(Inertia::redirect("/settings/two-factor"))?)
}

async fn confirmed_user<'a>(
    auth: &'a AuthSession,
    _request: &InertiaRequest,
) -> Result<Option<&'a crate::auth::User>, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(None);
    };
    if !password_is_confirmed(auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/two-factor")
            .await?;
        return Ok(None);
    }
    Ok(Some(user))
}
