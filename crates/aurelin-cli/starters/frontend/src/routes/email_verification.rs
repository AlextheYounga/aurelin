use aurelin_framework::{
    axum::{
        Router,
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use serde::Serialize;

use crate::{
    auth::AuthSession,
    frontend::Frontend,
    frontend_config::VERIFY_EMAIL_COMPONENT,
    routes::{
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
        .route("/verify-email", get(verify_email_notice))
        .route(
            "/email/verification-notification",
            post(send_verification_notification),
        )
        .route("/verify-email/{user_id}/{token}", get(verify_email))
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
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if user.email_verified_at.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let token = auth
        .backend
        .create_email_verification_token(user.id)
        .await?;
    tracing::info!(
        verification_url = %format!("/verify-email/{}/{token}", user.id),
        "email verification notification"
    );
    auth.session
        .insert(FLASH_STATUS, "verification-link-sent")
        .await?;
    Ok(request.redirect(Inertia::redirect("/verify-email"))?)
}

async fn verify_email(
    auth: AuthSession,
    request: InertiaRequest,
    Path((user_id, token)): Path<(i64, String)>,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if user.id != user_id {
        return Ok((StatusCode::FORBIDDEN, "Invalid verification link").into_response());
    }
    if user.email_verified_at.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    if !auth.backend.verify_email(user_id, &token).await? {
        return Ok((
            StatusCode::FORBIDDEN,
            "Invalid or expired verification link",
        )
            .into_response());
    }
    Ok(request.redirect(Inertia::redirect("/dashboard?verified=1"))?)
}
