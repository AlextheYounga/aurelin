use std::time::{SystemTime, UNIX_EPOCH};

use axum_tower_sessions_csrf::get_or_create_token;
use axum_valid::{Valid, ValidationRejection};
use kiln_framework::{
    axum::{
        Json, Router,
        extract::{State, rejection::JsonRejection},
        response::{IntoResponse, Response},
        routing::get,
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationErrors as ValidatorInputErrors};

use crate::{
    auth::{AuthSession, Credentials},
    frontend::Frontend,
    frontend_config::CONFIRM_PASSWORD_COMPONENT,
    routes::{
        session_feedback::{FLASH_ERRORS, ValidationErrors, redirect_with_error},
        web_error::WebError,
    },
};

const PASSWORD_CONFIRMED_AT: &str = "auth.password_confirmed_at";

#[derive(Serialize)]
struct ConfirmPasswordProps {
    errors: ValidationErrors,
    csrf_token: String,
}

#[derive(Deserialize, Validate)]
struct ConfirmPasswordInput {
    #[validate(length(min = 1, max = 1024))]
    password: String,
}

pub fn routes() -> Router<Frontend> {
    Router::new().route(
        "/confirm-password",
        get(confirm_password).post(store_password_confirmation),
    )
}

async fn confirm_password(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    if auth.user.is_none() {
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
            CONFIRM_PASSWORD_COMPONENT,
            ConfirmPasswordProps { errors, csrf_token },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn store_password_confirmation(
    auth: AuthSession,
    request: InertiaRequest,
    input: Result<
        Valid<Json<ConfirmPasswordInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return redirect_with_error(
                &auth,
                &request,
                "password",
                "The provided password is incorrect.",
                "/confirm-password",
            )
            .await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    if auth
        .authenticate(Credentials {
            email: user.email.clone(),
            password: input.password,
        })
        .await?
        .is_none()
    {
        return redirect_with_error(
            &auth,
            &request,
            "password",
            "The provided password is incorrect.",
            "/confirm-password",
        )
        .await;
    }

    let confirmed_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    auth.session
        .insert(PASSWORD_CONFIRMED_AT, confirmed_at)
        .await?;
    Ok(request.redirect(Inertia::redirect("/dashboard"))?)
}
