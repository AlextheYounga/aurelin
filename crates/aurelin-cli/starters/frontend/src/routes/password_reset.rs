use aurelin_framework::{
    auth::PasswordResetRequest,
    axum::{
        Json, Router,
        extract::{Path, Query, State, rejection::JsonRejection},
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use axum_valid::{Valid, ValidationRejection};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationErrors as ValidatorInputErrors};

use crate::{
    auth::AuthSession,
    frontend::Frontend,
    frontend_config::{FORGOT_PASSWORD_COMPONENT, RESET_PASSWORD_COMPONENT},
    routes::{
        session_feedback::{FLASH_ERRORS, FLASH_STATUS, ValidationErrors, redirect_with_error},
        web_error::WebError,
    },
};

#[derive(Serialize)]
struct PasswordRequestProps {
    errors: ValidationErrors,
    csrf_token: String,
    status: Option<String>,
}

#[derive(Serialize)]
struct ResetPasswordProps {
    errors: ValidationErrors,
    csrf_token: String,
    email: String,
    token: String,
    #[serde(rename = "passwordRules")]
    password_rules: &'static str,
}

#[derive(Deserialize, Validate)]
struct ForgotPasswordInput {
    #[validate(email, length(max = 255))]
    email: String,
}

#[derive(Deserialize, Validate)]
struct ResetPasswordInput {
    #[validate(email, length(max = 255))]
    email: String,
    #[validate(length(min = 1))]
    token: String,
    #[validate(length(min = 8))]
    password: String,
    password_confirmation: String,
}

#[derive(Deserialize, Validate)]
struct ResetPasswordQuery {
    #[validate(email, length(max = 255))]
    email: String,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route(
            "/forgot-password",
            get(forgot_password).post(send_password_reset_link),
        )
        .route("/reset-password/{token}", get(reset_password))
        .route("/reset-password", post(store_reset_password))
}

async fn forgot_password(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let errors = auth
        .session
        .remove::<ValidationErrors>(FLASH_ERRORS)
        .await?
        .unwrap_or_default();
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    Ok(request.render(
        Inertia::response(
            FORGOT_PASSWORD_COMPONENT,
            PasswordRequestProps {
                errors,
                csrf_token,
                status,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn send_password_reset_link(
    auth: AuthSession,
    request: InertiaRequest,
    input: Result<
        Valid<Json<ForgotPasswordInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return redirect_with_error(
                &auth,
                &request,
                "email",
                "The email must be a valid email address.",
                "/forgot-password",
            )
            .await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    match auth
        .backend
        .create_password_reset_token(&input.email)
        .await?
    {
        PasswordResetRequest::Sent { token } => {
            auth.backend
                .send_password_reset(&input.email, &token)
                .await?;
        }
        PasswordResetRequest::UnknownUser => {
            return redirect_with_error(
                &auth,
                &request,
                "email",
                "We can't find a user with that email address.",
                "/forgot-password",
            )
            .await;
        }
        PasswordResetRequest::Throttled => {
            return redirect_with_error(
                &auth,
                &request,
                "email",
                "Please wait before retrying.",
                "/forgot-password",
            )
            .await;
        }
    }
    auth.session
        .insert(FLASH_STATUS, "We have emailed your password reset link.")
        .await?;
    Ok(request.redirect(Inertia::redirect("/forgot-password"))?)
}

async fn reset_password(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
    Path(token): Path<String>,
    Valid(Query(query)): Valid<Query<ResetPasswordQuery>>,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
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
            RESET_PASSWORD_COMPONENT,
            ResetPasswordProps {
                errors,
                csrf_token,
                email: query.email,
                token,
                password_rules: auth.backend.password_rules(),
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn store_reset_password(
    auth: AuthSession,
    request: InertiaRequest,
    input: Result<Json<ResetPasswordInput>, JsonRejection>,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    let destination = format!(
        "/reset-password/{}?email={}",
        input.token,
        urlencoding::encode(&input.email)
    );
    if input.validate().is_err() || input.password != input.password_confirmation {
        return redirect_with_error(
            &auth,
            &request,
            "password",
            "The password must be at least 8 characters and match its confirmation.",
            &destination,
        )
        .await;
    }
    if let Err(violation) = auth.backend.validate_password(&input.password).await {
        return redirect_with_error(
            &auth,
            &request,
            "password",
            &violation.message(),
            &destination,
        )
        .await;
    }
    if !auth
        .backend
        .reset_password(&input.email, &input.token, &input.password)
        .await?
    {
        return redirect_with_error(
            &auth,
            &request,
            "email",
            "This password reset token is invalid.",
            &destination,
        )
        .await;
    }
    auth.session
        .insert(FLASH_STATUS, "Your password has been reset.")
        .await?;
    Ok(request.redirect(Inertia::redirect("/login"))?)
}
