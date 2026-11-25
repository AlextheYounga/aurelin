use crate::EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE;
use aurelin_framework::http as rate_limit;
use aurelin_framework::{
    auth::RateLimitOutcome,
    axum::{
        Router,
        extract::{Path, Query, State},
        http::{HeaderMap, StatusCode, Uri, header},
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_login::AuthUser;
use axum_tower_sessions_csrf::get_or_create_token;
use serde::{Deserialize, Serialize};

use super::{
    AuthSession, EmailVerificationRequest, FortifyBackend, FortifyFrontend, FortifyHttpError, FortifyUser,
    events::{self, Verified},
    feedback::{FLASH_STATUS, ValidationErrors},
};

#[derive(Serialize)]
struct VerifyEmailProps {
    errors: ValidationErrors,
    csrf_token: String,
    status: Option<String>,
}

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
{
    Router::new()
        .route("/email/verify", get(verify_email_notice::<F, B>))
        .route("/email/verification-notification", post(send_verification_notification::<B>))
        .route("/email/verify/{user_id}/{email_hash}", get(verify_email::<B>))
}

#[derive(Deserialize)]
struct VerificationQuery {
    expires: i64,
    signature: String,
}

async fn verify_email_notice<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if user.email_is_verified() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    Ok(request.render(
        Inertia::response(
            frontend.fortify_components().verify_email,
            VerifyEmailProps { errors: ValidationErrors::new(), csrf_token, status },
        ),
        move |context| frontend.fortify_document(context),
    )?)
}

async fn send_verification_notification<B: FortifyBackend>(
    auth: AuthSession<B>,
    headers: HeaderMap,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let remaining = match auth.backend.email_verification_attempt(user.id()).await {
        RateLimitOutcome::Allowed { remaining } => remaining,
        RateLimitOutcome::Limited { retry_after } => {
            return Ok(rate_limit::too_many_attempts(EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE, retry_after));
        }
    };
    if user.email_is_verified() {
        let response = request.redirect(Inertia::redirect("/dashboard"))?;
        return Ok(with_rate_limit_headers(response, remaining));
    }
    auth.backend.send_email_verification(user).await.map_err(FortifyHttpError::backend)?;
    auth.session.insert(FLASH_STATUS, "verification-link-sent").await?;
    let destination = previous_location(&headers).unwrap_or_else(|| "/".into());
    let response = request.redirect(Inertia::redirect(&destination))?;
    Ok(with_rate_limit_headers(response, remaining))
}

fn previous_location(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::REFERER)?
        .to_str()
        .ok()?
        .parse::<Uri>()
        .ok()?
        .path_and_query()
        .map(|path| path.as_str().to_owned())
}

async fn verify_email<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
    Path((user_id, email_hash)): Path<(i64, String)>,
    Query(query): Query<VerificationQuery>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let remaining = match auth.backend.email_verification_attempt(user.id()).await {
        RateLimitOutcome::Allowed { remaining } => remaining,
        RateLimitOutcome::Limited { retry_after } => {
            return Ok(rate_limit::too_many_attempts(EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE, retry_after));
        }
    };
    if user.id() != user_id {
        let response = (StatusCode::FORBIDDEN, "Invalid verification link").into_response();
        return Ok(with_rate_limit_headers(response, remaining));
    }
    if user.email_is_verified() {
        let response = request.redirect(Inertia::redirect("/dashboard?verified=1"))?;
        return Ok(with_rate_limit_headers(response, remaining));
    }
    if !auth
        .backend
        .verify_email(EmailVerificationRequest {
            user,
            email_hash: &email_hash,
            expires: query.expires,
            signature: &query.signature,
        })
        .await
        .map_err(FortifyHttpError::backend)?
    {
        let response = (StatusCode::FORBIDDEN, "Invalid or expired verification link").into_response();
        return Ok(with_rate_limit_headers(response, remaining));
    }
    events::observe(auth.backend.authentication_events(), Verified { user_id: user.id() }).await;
    let response = request.redirect(Inertia::redirect("/dashboard?verified=1"))?;
    Ok(with_rate_limit_headers(response, remaining))
}

fn with_rate_limit_headers(mut response: Response, remaining: u32) -> Response {
    rate_limit::add_rate_limit_headers(&mut response, EMAIL_VERIFICATION_ATTEMPTS_PER_MINUTE, remaining, None);
    response
}
