use aurelin_fortify::EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE;
use aurelin_framework::{
    auth::RateLimitOutcome,
    axum::{
        Router,
        extract::{Path, Query, State},
        http::{HeaderMap, StatusCode, header},
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
    frontend_config::VERIFY_EMAIL_COMPONENT,
    routes::{
        rate_limit,
        session_feedback::{FLASH_STATUS, ValidationErrors},
        web_error::WebError,
    },
};

#[derive(Serialize)]
struct VerifyEmailProps {
    errors: ValidationErrors,
    csrf_token: String,
    status: Option<String>,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/email/verify", get(verify_email_notice))
        .route(
            "/email/verification-notification",
            post(send_verification_notification),
        )
        .route("/email/verify/{user_id}/{email_hash}", get(verify_email))
}

#[derive(Deserialize)]
struct VerificationQuery {
    expires: i64,
    signature: String,
}

async fn verify_email_notice(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if user.email_verified_at.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    Ok(request.render(
        Inertia::response(
            VERIFY_EMAIL_COMPONENT,
            VerifyEmailProps {
                errors: ValidationErrors::new(),
                csrf_token,
                status,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn send_verification_notification(
    auth: AuthSession,
    headers: HeaderMap,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let remaining = match auth.backend.email_verification_attempt(user.id).await {
        RateLimitOutcome::Allowed { remaining } => remaining,
        RateLimitOutcome::Limited { retry_after } => {
            return Ok(rate_limit::too_many_attempts(
                EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE,
                retry_after,
            ));
        }
    };
    if user.email_verified_at.is_some() {
        let response = request.redirect(Inertia::redirect("/dashboard"))?;
        return Ok(with_rate_limit_headers(response, remaining));
    }
    auth.backend.send_email_verification(user).await?;
    auth.session
        .insert(FLASH_STATUS, "verification-link-sent")
        .await?;
    let destination = previous_location(&headers).unwrap_or_else(|| "/".into());
    let response = request.redirect(Inertia::redirect(&destination))?;
    Ok(with_rate_limit_headers(response, remaining))
}

fn previous_location(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::REFERER)?
        .to_str()
        .ok()?
        .parse::<aurelin_framework::axum::http::Uri>()
        .ok()?
        .path_and_query()
        .map(|path| path.as_str().to_owned())
}

async fn verify_email(
    auth: AuthSession,
    request: InertiaRequest,
    Path((user_id, email_hash)): Path<(i64, String)>,
    Query(query): Query<VerificationQuery>,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let remaining = match auth.backend.email_verification_attempt(user.id).await {
        RateLimitOutcome::Allowed { remaining } => remaining,
        RateLimitOutcome::Limited { retry_after } => {
            return Ok(rate_limit::too_many_attempts(
                EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE,
                retry_after,
            ));
        }
    };
    if user.id != user_id {
        let response = (StatusCode::FORBIDDEN, "Invalid verification link").into_response();
        return Ok(with_rate_limit_headers(response, remaining));
    }
    if user.email_verified_at.is_some() {
        let response = request.redirect(Inertia::redirect("/dashboard?verified=1"))?;
        return Ok(with_rate_limit_headers(response, remaining));
    }
    if !auth
        .backend
        .verify_email(user, &email_hash, query.expires, &query.signature)
        .await?
    {
        let response = (
            StatusCode::FORBIDDEN,
            "Invalid or expired verification link",
        )
            .into_response();
        return Ok(with_rate_limit_headers(response, remaining));
    }
    let response = request.redirect(Inertia::redirect("/dashboard?verified=1"))?;
    Ok(with_rate_limit_headers(response, remaining))
}

fn with_rate_limit_headers(mut response: Response, remaining: u32) -> Response {
    rate_limit::add_headers(
        &mut response,
        EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE,
        remaining,
        None,
    );
    response
}
