use std::time::{SystemTime, UNIX_EPOCH};

use aurelin_framework::{
    axum::{
        Json, Router,
        extract::{Query, State, rejection::JsonRejection},
        http::HeaderValue,
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
    auth::{AuthSession, Credentials},
    frontend::Frontend,
    frontend_config::CONFIRM_PASSWORD_COMPONENT,
    routes::{
        session_feedback::{FLASH_ERRORS, ValidationErrors, redirect_with_error},
        web_error::WebError,
    },
};

const PASSWORD_CONFIRMED_AT: &str = "auth.password_confirmed_at";
const DEFAULT_PASSWORD_TIMEOUT_SECONDS: u64 = 10_800;

#[derive(Serialize)]
struct ConfirmPasswordProps {
    errors: ValidationErrors,
    csrf_token: String,
}

#[derive(Deserialize, Validate)]
struct ConfirmPasswordInput {
    #[validate(length(min = 1))]
    password: String,
}

#[derive(Deserialize)]
struct ConfirmationStatusQuery {
    seconds: Option<u64>,
}

#[derive(Serialize)]
struct ConfirmationStatus {
    confirmed: bool,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route(
            "/user/confirm-password",
            get(confirm_password).post(store_password_confirmation),
        )
        .route(
            "/user/confirmed-password-status",
            get(confirmed_password_status),
        )
}

async fn confirmed_password_status(
    auth: AuthSession,
    Query(query): Query<ConfirmationStatusQuery>,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    if auth.user.is_none() {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    let confirmed_at = auth
        .session
        .get::<u64>(PASSWORD_CONFIRMED_AT)
        .await?
        .unwrap_or_default();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let elapsed = now.saturating_sub(confirmed_at);
    let confirmed = elapsed < query.seconds.unwrap_or(DEFAULT_PASSWORD_TIMEOUT_SECONDS);
    let mut response = Json(ConfirmationStatus { confirmed }).into_response();
    if confirmed && let Ok(value) = HeaderValue::from_str(&elapsed.to_string()) {
        response.headers_mut().insert("x-retry-after", value);
    }
    Ok(response)
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
                "/user/confirm-password",
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
            "/user/confirm-password",
        )
        .await;
    }

    let confirmed_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    auth.session
        .insert(PASSWORD_CONFIRMED_AT, confirmed_at)
        .await?;
    Ok(request.redirect(Inertia::redirect("/dashboard"))?)
}
