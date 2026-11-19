use aurelin_framework::http as rate_limit;
use aurelin_framework::{
    auth::RateLimitOutcome,
    axum::{
        Json, Router,
        extract::{State, rejection::JsonRejection},
        response::{IntoResponse, Response},
        routing::{get, put},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use axum_valid::{Valid, ValidationRejection};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationErrors as ValidatorInputErrors};

use crate::{
    auth::{AuthSession, ProfileUpdate, PublicUser},
    frontend::Frontend,
    frontend_config::PROFILE_SETTINGS_COMPONENT,
    routes::{
        session_feedback::{FLASH_ERRORS, FLASH_STATUS, ValidationErrors, redirect_with_error},
        web_error::WebError,
    },
};

mod appearance;
mod security;

const PASSWORD_UPDATE_ATTEMPTS_PER_MINUTE: u32 = 6;

#[derive(Serialize)]
struct ProfileProps {
    auth: SettingsAuth,
    errors: ValidationErrors,
    status: Option<String>,
    csrf_token: String,
    #[serde(rename = "mustVerifyEmail")]
    must_verify_email: bool,
}

#[derive(Serialize)]
struct SettingsAuth {
    user: PublicUser,
}

#[derive(Deserialize, Validate)]
struct ProfileInput {
    #[validate(length(min = 1, max = 255))]
    name: String,
    #[validate(email, length(max = 255))]
    email: String,
}

#[derive(Deserialize, Validate)]
struct PasswordInput {
    #[validate(length(min = 1))]
    current_password: String,
    #[validate(length(min = 8))]
    password: String,
    password_confirmation: String,
}

#[derive(Deserialize, Validate)]
struct DeleteAccountInput {
    #[validate(length(min = 1))]
    password: String,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/settings", get(settings))
        .route(
            "/settings/profile",
            get(profile).patch(update_profile).delete(delete_account),
        )
        .route("/settings/security", get(security::show))
        .route("/settings/password", put(update_password))
        .route("/settings/appearance", get(appearance::show))
}

async fn settings(auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    if auth.user.is_none() {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    Ok(request.redirect(Inertia::redirect("/settings/profile"))?)
}

async fn profile(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let errors = take_errors(&auth).await?;
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    Ok(request.render(
        Inertia::response(
            PROFILE_SETTINGS_COMPONENT,
            ProfileProps {
                auth: SettingsAuth {
                    user: PublicUser::from(user),
                },
                errors,
                status,
                csrf_token,
                must_verify_email: true,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn update_profile(
    auth: AuthSession,
    request: InertiaRequest,
    input: Result<
        Valid<Json<ProfileInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(validation)) => {
            auth.session
                .insert(FLASH_ERRORS, profile_validation_errors(&validation))
                .await?;
            return Ok(request.redirect(Inertia::redirect("/settings/profile"))?);
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    match auth
        .backend
        .update_profile(user, input.name, input.email)
        .await?
    {
        ProfileUpdate::Updated(_) => {
            auth.session.insert(FLASH_STATUS, "profile-updated").await?;
            Ok(request.redirect(Inertia::redirect("/settings/profile"))?)
        }
        ProfileUpdate::EmailAlreadyExists => {
            redirect_with_error(
                &auth,
                &request,
                "email",
                "The email has already been taken.",
                "/settings/profile",
            )
            .await
        }
    }
}

async fn delete_account(
    mut auth: AuthSession,
    request: InertiaRequest,
    input: Result<
        Valid<Json<DeleteAccountInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if user.email_verified_at.is_none() {
        return Ok(request.redirect(Inertia::redirect("/email/verify"))?);
    }
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return redirect_with_error(
                &auth,
                &request,
                "password",
                "The password field is required.",
                "/settings/profile",
            )
            .await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    if !auth.backend.delete_account(user, &input.password).await? {
        return redirect_with_error(
            &auth,
            &request,
            "password",
            "The password is incorrect.",
            "/settings/profile",
        )
        .await;
    }
    auth.logout().await?;
    auth.session.flush().await?;
    Ok(request.redirect(Inertia::redirect("/"))?)
}

async fn update_password(
    mut auth: AuthSession,
    request: InertiaRequest,
    input: Result<
        Valid<Json<PasswordInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let Some(user) = verified_user(&auth) else {
        return Ok(request.redirect(Inertia::redirect(unauthenticated_destination(&auth)))?);
    };
    let remaining = match auth.backend.password_update_attempt(user.id).await {
        RateLimitOutcome::Allowed { remaining } => remaining,
        RateLimitOutcome::Limited { retry_after } => {
            return Ok(rate_limit::too_many_attempts(
                PASSWORD_UPDATE_ATTEMPTS_PER_MINUTE,
                retry_after,
            ));
        }
    };
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(validation)) => {
            auth.session
                .insert(FLASH_ERRORS, password_validation_errors(&validation))
                .await?;
            let response = request.redirect(Inertia::redirect("/settings/security"))?;
            return Ok(with_password_update_rate_limit(response, remaining));
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    if input.password != input.password_confirmation {
        let response = redirect_with_error(
            &auth,
            &request,
            "password",
            "The password confirmation does not match.",
            "/settings/security",
        )
        .await?;
        return Ok(with_password_update_rate_limit(response, remaining));
    }
    if let Err(violation) = auth.backend.validate_password(&input.password).await {
        let response = redirect_with_error(
            &auth,
            &request,
            "password",
            &violation.message(),
            "/settings/security",
        )
        .await?;
        return Ok(with_password_update_rate_limit(response, remaining));
    }
    let Some(updated_user) = auth
        .backend
        .update_password(user, &input.current_password, &input.password)
        .await?
    else {
        let response = redirect_with_error(
            &auth,
            &request,
            "current_password",
            "The password is incorrect.",
            "/settings/security",
        )
        .await?;
        return Ok(with_password_update_rate_limit(response, remaining));
    };
    auth.login(&updated_user).await?;
    auth.session
        .insert(FLASH_STATUS, "password-updated")
        .await?;
    let response = request.redirect(Inertia::redirect("/settings/security"))?;
    Ok(with_password_update_rate_limit(response, remaining))
}

fn with_password_update_rate_limit(mut response: Response, remaining: u32) -> Response {
    rate_limit::add_rate_limit_headers(
        &mut response,
        PASSWORD_UPDATE_ATTEMPTS_PER_MINUTE,
        remaining,
        None,
    );
    response
}

pub(super) fn verified_user(auth: &AuthSession) -> Option<&crate::auth::User> {
    auth.user
        .as_ref()
        .filter(|user| user.email_verified_at.is_some())
}

pub(super) fn unauthenticated_destination(auth: &AuthSession) -> &'static str {
    if auth.user.is_some() {
        "/email/verify"
    } else {
        "/login"
    }
}

pub(super) async fn take_errors(auth: &AuthSession) -> Result<ValidationErrors, WebError> {
    Ok(auth
        .session
        .remove::<ValidationErrors>(FLASH_ERRORS)
        .await?
        .unwrap_or_default())
}

fn profile_validation_errors(validation: &ValidatorInputErrors) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    if validation.field_errors().contains_key("name") {
        errors.insert(
            "name".into(),
            "The name must be between 1 and 255 characters.".into(),
        );
    }
    if validation.field_errors().contains_key("email") {
        errors.insert(
            "email".into(),
            "The email must be a valid email address.".into(),
        );
    }
    errors
}

fn password_validation_errors(validation: &ValidatorInputErrors) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    if validation.field_errors().contains_key("current_password") {
        errors.insert(
            "current_password".into(),
            "The current password field is required.".into(),
        );
    }
    if validation.field_errors().contains_key("password") {
        errors.insert(
            "password".into(),
            "The password must be at least 8 characters.".into(),
        );
    }
    errors
}
