use std::net::SocketAddr;

use aurelin_framework::{
    axum::{
        Json, Router,
        extract::{ConnectInfo, State, rejection::JsonRejection},
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
    auth::{AuthSession, CreateUser, Credentials, PublicUser},
    frontend::Frontend,
    frontend_config::{DASHBOARD_COMPONENT, LOGIN_COMPONENT, REGISTER_COMPONENT},
    routes::{
        session_feedback::{FLASH_ERRORS, FLASH_STATUS, ValidationErrors, redirect_with_error},
        web_error::WebError,
    },
};

#[derive(Serialize)]
struct AuthPageProps {
    errors: ValidationErrors,
    csrf_token: String,
    status: Option<String>,
}

#[derive(Serialize)]
struct DashboardProps {
    auth: DashboardAuth,
    csrf_token: String,
}

#[derive(Serialize)]
struct DashboardAuth {
    user: PublicUser,
}

#[derive(Deserialize, Validate)]
struct LoginInput {
    #[validate(email, length(max = 255))]
    email: String,
    #[validate(length(min = 1, max = 1024))]
    password: String,
}

#[derive(Deserialize, Validate)]
struct RegistrationInput {
    #[validate(length(min = 1, max = 255))]
    name: String,
    #[validate(email, length(max = 255))]
    email: String,
    #[validate(length(min = 8, max = 1024))]
    password: String,
    password_confirmation: String,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/dashboard", get(dashboard))
        .route("/login", get(login).post(store_login))
        .route("/register", get(register).post(store_registration))
        .route("/logout", post(logout))
}

async fn login(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    render_auth_page(frontend, auth, request, LOGIN_COMPONENT).await
}

async fn register(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    render_auth_page(frontend, auth, request, REGISTER_COMPONENT).await
}

async fn render_auth_page(
    frontend: Frontend,
    auth: AuthSession,
    request: InertiaRequest,
    component: &'static str,
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
            component,
            AuthPageProps {
                errors,
                csrf_token,
                status,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn store_login(
    mut auth: AuthSession,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    request: InertiaRequest,
    input: Result<
        Valid<Json<LoginInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return invalid_credentials(&auth, &request).await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    let key = format!("{}|{}", input.email.to_lowercase(), address.ip());
    if auth.backend.too_many_login_attempts(&key).await {
        return redirect_with_error(
            &auth,
            &request,
            "email",
            "Too many login attempts. Please try again in 60 seconds.",
            "/login",
        )
        .await;
    }
    let user = auth
        .authenticate(Credentials {
            email: input.email,
            password: input.password,
        })
        .await?;
    let Some(user) = user else {
        tracing::warn!("authentication failed");
        return invalid_credentials(&auth, &request).await;
    };
    auth.backend.clear_login_attempts(&key).await;
    auth.login(&user).await?;
    Ok(request.redirect(Inertia::redirect("/dashboard"))?)
}

async fn invalid_credentials(
    auth: &AuthSession,
    request: &InertiaRequest,
) -> Result<Response, WebError> {
    redirect_with_error(
        auth,
        request,
        "email",
        "These credentials do not match our records.",
        "/login",
    )
    .await
}

async fn store_registration(
    mut auth: AuthSession,
    request: InertiaRequest,
    input: Result<
        Valid<Json<RegistrationInput>>,
        ValidationRejection<ValidatorInputErrors, JsonRejection>,
    >,
) -> Result<Response, WebError> {
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(validation)) => {
            let errors = registration_validation_errors(&validation);
            auth.session.insert(FLASH_ERRORS, errors).await?;
            return Ok(request.redirect(Inertia::redirect("/register"))?);
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    let errors = registration_confirmation_errors(&input);
    if !errors.is_empty() {
        auth.session.insert(FLASH_ERRORS, errors).await?;
        return Ok(request.redirect(Inertia::redirect("/register"))?);
    }
    let user = match auth
        .backend
        .create_user(&input.name, &input.email, &input.password)
        .await?
    {
        CreateUser::Created(user) => user,
        CreateUser::EmailAlreadyExists => {
            return redirect_with_error(
                &auth,
                &request,
                "email",
                "The email has already been taken.",
                "/register",
            )
            .await;
        }
    };
    let token = auth
        .backend
        .create_email_verification_token(user.id)
        .await?;
    tracing::info!(
        verification_url = %format!("/verify-email/{}/{token}", user.id),
        "email verification notification"
    );
    auth.login(&user).await?;
    tracing::info!(user_id = user.id, "user registered");
    Ok(request.redirect(Inertia::redirect("/dashboard"))?)
}

async fn logout(mut auth: AuthSession, request: InertiaRequest) -> Result<Response, WebError> {
    auth.logout().await?;
    Ok(request.redirect(Inertia::redirect("/"))?)
}

async fn dashboard(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if user.email_verified_at.is_none() {
        return Ok(request.redirect(Inertia::redirect("/verify-email"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    Ok(request.render(
        Inertia::response(
            DASHBOARD_COMPONENT,
            DashboardProps {
                auth: DashboardAuth {
                    user: PublicUser::from(user),
                },
                csrf_token,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

fn registration_validation_errors(validation: &ValidatorInputErrors) -> ValidationErrors {
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
    if validation.field_errors().contains_key("password") {
        errors.insert(
            "password".into(),
            "The password must be at least 8 characters.".into(),
        );
    }
    errors
}

fn registration_confirmation_errors(input: &RegistrationInput) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    if input.password != input.password_confirmation {
        errors.insert(
            "password".into(),
            "The password confirmation does not match.".into(),
        );
    }
    errors
}
