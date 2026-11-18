use aurelin_framework::{
    axum::{
        Json, Router,
        extract::{
            Path, Query, State,
            rejection::{JsonRejection, QueryRejection},
        },
        response::{IntoResponse, Response},
        routing::get,
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
}

#[derive(Deserialize, Validate)]
struct ForgotPasswordInput {
    #[validate(email, length(max = 255))]
    email: String,
}

#[derive(Deserialize, Validate)]
struct ResetPasswordInput {
    #[validate(length(min = 8, max = 1024))]
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
        .route(
            "/reset-password/{token}",
            get(reset_password).post(store_reset_password),
        )
}

async fn forgot_password(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
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
    if let Some(token) = auth
        .backend
        .create_password_reset_token(&input.email)
        .await?
    {
        let email = urlencoding::encode(&input.email);
        tracing::info!(reset_url = %format!("/reset-password/{token}?email={email}"), "password reset notification");
    }
    auth.session
        .insert(
            FLASH_STATUS,
            "If an account exists for that email, a password reset link has been sent.",
        )
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
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn store_reset_password(
    auth: AuthSession,
    request: InertiaRequest,
    Path(token): Path<String>,
    query: Result<
        Valid<Query<ResetPasswordQuery>>,
        ValidationRejection<ValidatorInputErrors, QueryRejection>,
    >,
    input: Result<
        Valid<Json<ResetPasswordInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let query = match query {
        Ok(Valid(Query(query))) => query,
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
    let destination = format!(
        "/reset-password/{}?email={}",
        token,
        urlencoding::encode(&query.email)
    );
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return redirect_with_error(
                &auth,
                &request,
                "password",
                "The password must be at least 8 characters and match its confirmation.",
                &destination,
            )
            .await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    if input.password != input.password_confirmation {
        return redirect_with_error(
            &auth,
            &request,
            "password",
            "The password must be at least 8 characters and match its confirmation.",
            &destination,
        )
        .await;
    }
    if !auth
        .backend
        .reset_password(&query.email, &token, &input.password)
        .await?
    {
        return redirect_with_error(
            &auth,
            &request,
            "email",
            "This password reset token is invalid or has expired.",
            &destination,
        )
        .await;
    }
    auth.session
        .insert(FLASH_STATUS, "Your password has been reset.")
        .await?;
    Ok(request.redirect(Inertia::redirect("/login"))?)
}
