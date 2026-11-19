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

use super::{
    AuthSession, Credentials, FortifyBackend, FortifyFrontend, FortifyHttpError, FortifyUser,
    feedback::{FLASH_ERRORS, ValidationErrors, redirect_with_error},
};

pub const PASSWORD_CONFIRMED_AT: &str = "auth.password_confirmed_at";
pub const PASSWORD_CONFIRMATION_REDIRECT: &str = "auth.password_confirmation_redirect";
pub const DEFAULT_PASSWORD_TIMEOUT_SECONDS: u64 = 10_800;

pub async fn password_is_confirmed<B: FortifyBackend>(auth: &AuthSession<B>) -> Result<bool, FortifyHttpError> {
    let confirmed_at = auth.session.get::<u64>(PASSWORD_CONFIRMED_AT).await?.unwrap_or_default();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(now.saturating_sub(confirmed_at) < DEFAULT_PASSWORD_TIMEOUT_SECONDS)
}

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

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
{
    Router::new()
        .route("/user/confirm-password", get(confirm_password::<F, B>).post(store_password_confirmation::<B>))
        .route("/user/confirmed-password-status", get(confirmed_password_status::<B>))
}

async fn confirmed_password_status<B: FortifyBackend>(
    auth: AuthSession<B>,
    Query(query): Query<ConfirmationStatusQuery>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_none() {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    let confirmed_at = auth.session.get::<u64>(PASSWORD_CONFIRMED_AT).await?.unwrap_or_default();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let elapsed = now.saturating_sub(confirmed_at);
    let confirmed = elapsed < query.seconds.unwrap_or(DEFAULT_PASSWORD_TIMEOUT_SECONDS);
    let mut response = Json(ConfirmationStatus { confirmed }).into_response();
    if confirmed && let Ok(value) = HeaderValue::from_str(&elapsed.to_string()) {
        response.headers_mut().insert("x-retry-after", value);
    }
    Ok(response)
}

async fn confirm_password<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_none() {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    let errors = auth.session.remove::<ValidationErrors>(FLASH_ERRORS).await?.unwrap_or_default();
    Ok(request.render(
        Inertia::response(frontend.fortify_components().confirm_password, ConfirmPasswordProps { errors, csrf_token }),
        move |context| frontend.fortify_document(context),
    )?)
}

async fn store_password_confirmation<B: FortifyBackend>(
    auth: AuthSession<B>,
    request: InertiaRequest,
    input: Result<Valid<Json<ConfirmPasswordInput>>, ValidationRejection<ValidatorInputErrors, JsonRejection>>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return redirect_with_error(
                &auth,
                &request,
                ("password", "The provided password is incorrect."),
                "/user/confirm-password",
            )
            .await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    if auth
        .authenticate(Credentials { email: user.email().to_owned(), password: input.password })
        .await
        .map_err(FortifyHttpError::auth_session)?
        .is_none()
    {
        return redirect_with_error(
            &auth,
            &request,
            ("password", "The provided password is incorrect."),
            "/user/confirm-password",
        )
        .await;
    }

    let confirmed_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    auth.session.insert(PASSWORD_CONFIRMED_AT, confirmed_at).await?;
    let destination =
        auth.session.remove::<String>(PASSWORD_CONFIRMATION_REDIRECT).await?.unwrap_or_else(|| "/dashboard".into());
    Ok(request.redirect(Inertia::redirect(&destination))?)
}
