use aurelin_framework::{
    auth::PasswordResetRequest,
    axum::{
        Json, Router,
        extract::{Path, Query, State, rejection::JsonRejection},
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
    timebox::Timebox,
};
use axum_tower_sessions_csrf::get_or_create_token;
use axum_valid::{Valid, ValidationRejection};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationErrors as ValidatorInputErrors};

use super::{
    AuthSession, FortifyBackend, FortifyFrontend, FortifyHttpError,
    events::{self, PasswordReset},
    feedback::{FLASH_ERRORS, FLASH_STATUS, ValidationErrors, redirect_with_error},
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

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
{
    Router::new()
        .route("/forgot-password", get(forgot_password::<F, B>).post(send_password_reset_link::<B>))
        .route("/reset-password/{token}", get(reset_password::<F, B>))
        .route("/reset-password", post(store_reset_password::<B>))
}

async fn forgot_password<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    let errors = auth.session.remove::<ValidationErrors>(FLASH_ERRORS).await?.unwrap_or_default();
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    Ok(request.render(
        Inertia::response(
            frontend.fortify_components().forgot_password,
            PasswordRequestProps { errors, csrf_token, status },
        ),
        move |context| frontend.fortify_document(context),
    )?)
}

async fn send_password_reset_link<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
    input: Result<Valid<Json<ForgotPasswordInput>>, ValidationRejection<ValidatorInputErrors, JsonRejection>>,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return redirect_with_error(
                &auth,
                &request,
                ("email", "The email must be a valid email address."),
                "/forgot-password",
            )
            .await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    let timebox = Timebox::start(auth.backend.authentication_timebox_duration());
    match auth.backend.create_password_reset_token(&input.email).await {
        Ok(PasswordResetRequest::Sent { token }) => {
            if auth.backend.send_password_reset(&input.email, &token).await.is_err() {
                tracing::error!("password reset notification failed");
            }
        }
        Ok(PasswordResetRequest::UnknownUser | PasswordResetRequest::Throttled) => {}
        Err(_) => tracing::error!("password reset request failed"),
    }
    timebox.wait().await;
    auth.session.insert(FLASH_STATUS, "We have emailed your password reset link.").await?;
    Ok(request.redirect(Inertia::redirect("/forgot-password"))?)
}

async fn reset_password<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
    (Path(token), Valid(Query(query))): (Path<String>, Valid<Query<ResetPasswordQuery>>),
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    let errors = auth.session.remove::<ValidationErrors>(FLASH_ERRORS).await?.unwrap_or_default();
    Ok(request.render(
        Inertia::response(
            frontend.fortify_components().reset_password,
            ResetPasswordProps {
                errors,
                csrf_token,
                email: query.email,
                token,
                password_rules: auth.backend.password_rules(),
            },
        ),
        move |context| frontend.fortify_document(context),
    )?)
}

async fn store_reset_password<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
    input: Result<Json<ResetPasswordInput>, JsonRejection>,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    let destination = format!("/reset-password/{}?email={}", input.token, urlencoding::encode(&input.email));
    if input.validate().is_err() || input.password != input.password_confirmation {
        return redirect_with_error(
            &auth,
            &request,
            ("password", "The password must be at least 8 characters and match its confirmation."),
            &destination,
        )
        .await;
    }
    if let Err(message) = auth.backend.validate_password(&input.password).await {
        return redirect_with_error(&auth, &request, ("password", &message), &destination).await;
    }
    if !auth
        .backend
        .reset_password(&input.email, &input.token, &input.password)
        .await
        .map_err(FortifyHttpError::backend)?
    {
        return redirect_with_error(&auth, &request, ("email", "This password reset token is invalid."), &destination)
            .await;
    }
    events::observe(auth.backend.authentication_events(), PasswordReset { email: input.email }).await;
    auth.session.insert(FLASH_STATUS, "Your password has been reset.").await?;
    Ok(request.redirect(Inertia::redirect("/login"))?)
}
