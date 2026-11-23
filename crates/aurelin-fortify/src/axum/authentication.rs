use std::net::SocketAddr;

use crate::LOGIN_ATTEMPTS_PER_MINUTE;
use aurelin_framework::http as rate_limit;
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
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use validator::{Validate, ValidationErrors as ValidatorInputErrors};

use super::{
    AuthSession, FortifyBackend, FortifyFrontend, FortifyHttpError, FortifyUser,
    authentication_workflow::{
        LoginAttempt, LoginOutcome, RegistrationAttempt, RegistrationOutcome, attempt_login, attempt_registration,
        logout,
    },
    feedback::{FLASH_ERRORS, FLASH_STATUS, ValidationErrors, redirect_with_error},
};

#[derive(Serialize)]
struct AuthPageProps {
    errors: ValidationErrors,
    csrf_token: String,
    status: Option<String>,
    #[serde(rename = "passwordRules", skip_serializing_if = "Option::is_none")]
    password_rules: Option<&'static str>,
    #[serde(rename = "canResetPassword")]
    can_reset_password: bool,
}

#[derive(Serialize)]
struct DashboardProps<U: Serialize> {
    auth: DashboardAuth<U>,
    csrf_token: String,
}

#[derive(Serialize)]
struct DashboardAuth<U: Serialize> {
    user: U,
}

enum AuthPage {
    Login,
    Register,
}

impl AuthPage {
    fn component<F: FortifyFrontend>(&self, frontend: &F) -> &'static str {
        match self {
            Self::Login => frontend.fortify_components().login,
            Self::Register => frontend.fortify_components().register,
        }
    }

    fn includes_password_rules(&self) -> bool {
        matches!(self, Self::Register)
    }
}

#[derive(Deserialize, Validate)]
struct LoginInput {
    #[validate(email, length(max = 255))]
    email: String,
    #[validate(length(min = 1))]
    password: String,
    #[serde(default, deserialize_with = "deserialize_boolean")]
    remember: bool,
}

fn deserialize_boolean<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::Bool(value) => value,
        Value::Number(value) => value.as_i64() == Some(1),
        Value::String(value) => matches!(value.as_str(), "1" | "true" | "on" | "yes"),
        _ => false,
    })
}

#[derive(Deserialize, Validate)]
struct RegistrationInput {
    #[validate(length(min = 1, max = 255))]
    name: String,
    #[validate(email, length(max = 255))]
    email: String,
    #[validate(length(min = 8))]
    password: String,
    password_confirmation: String,
}

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
{
    Router::new()
        .route("/dashboard", get(dashboard::<F, B>))
        .route("/login", get(login::<F, B>).post(store_login::<B>))
        .route("/register", get(register::<F, B>).post(store_registration::<B>))
        .route("/logout", post(store_logout::<B>))
}

async fn login<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    render_auth_page(frontend, auth, request, AuthPage::Login).await
}

async fn register<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    render_auth_page(frontend, auth, request, AuthPage::Register).await
}

async fn render_auth_page<F: FortifyFrontend, B: FortifyBackend>(
    frontend: F,
    auth: AuthSession<B>,
    request: InertiaRequest,
    page: AuthPage,
) -> Result<Response, FortifyHttpError> {
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    let errors = auth.session.remove::<ValidationErrors>(FLASH_ERRORS).await?.unwrap_or_default();
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    Ok(request.render(
        Inertia::response(
            page.component(&frontend),
            AuthPageProps {
                errors,
                csrf_token,
                status,
                password_rules: page.includes_password_rules().then(|| auth.backend.password_rules()),
                can_reset_password: true,
            },
        ),
        move |context| frontend.fortify_document(context),
    )?)
}

async fn store_login<B: FortifyBackend>(
    mut auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    request: InertiaRequest,
    input: Result<Valid<Json<LoginInput>>, ValidationRejection<ValidatorInputErrors, JsonRejection>>,
) -> Result<Response, FortifyHttpError> {
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(_)) => {
            return invalid_credentials(&auth, &request).await;
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    match attempt_login(
        &mut auth,
        LoginAttempt {
            email: input.email,
            password: input.password,
            remember: input.remember,
            ip_address: address.ip(),
        },
    )
    .await?
    {
        LoginOutcome::AlreadyAuthenticated | LoginOutcome::Authenticated => {
            Ok(request.redirect(Inertia::redirect("/dashboard"))?)
        }
        LoginOutcome::TwoFactorChallenge => Ok(request.redirect(Inertia::redirect("/two-factor-challenge"))?),
        LoginOutcome::InvalidCredentials { remaining } => {
            let mut response = invalid_credentials(&auth, &request).await?;
            rate_limit::add_rate_limit_headers(&mut response, LOGIN_ATTEMPTS_PER_MINUTE, remaining, None);
            Ok(response)
        }
        LoginOutcome::RateLimited { retry_after } => {
            Ok(rate_limit::too_many_attempts(LOGIN_ATTEMPTS_PER_MINUTE, retry_after))
        }
    }
}

async fn invalid_credentials<B: FortifyBackend>(
    auth: &AuthSession<B>,
    request: &InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    redirect_with_error(auth, request, ("email", "These credentials do not match our records."), "/login").await
}

async fn store_registration<B: FortifyBackend>(
    mut auth: AuthSession<B>,
    request: InertiaRequest,
    input: Result<Valid<Json<RegistrationInput>>, ValidationRejection<ValidatorInputErrors, JsonRejection>>,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok(request.redirect(Inertia::redirect("/dashboard"))?);
    }
    let input = match input {
        Ok(Valid(Json(input))) => input,
        Err(ValidationRejection::Valid(validation)) => {
            let errors = registration_validation_errors(&validation);
            auth.session.insert(FLASH_ERRORS, errors).await?;
            return Ok(request.redirect(Inertia::redirect("/register"))?);
        }
        Err(ValidationRejection::Inner(error)) => return Ok(error.into_response()),
    };
    match attempt_registration(
        &mut auth,
        RegistrationAttempt {
            name: input.name,
            email: input.email,
            password: input.password,
            password_confirmation: input.password_confirmation,
        },
    )
    .await?
    {
        RegistrationOutcome::AlreadyAuthenticated | RegistrationOutcome::Created => {
            Ok(request.redirect(Inertia::redirect("/dashboard"))?)
        }
        RegistrationOutcome::EmailAlreadyExists => {
            return redirect_with_error(&auth, &request, ("email", "The email has already been taken."), "/register")
                .await;
        }
        RegistrationOutcome::InvalidInput { field, message } => {
            redirect_with_error(&auth, &request, (field, &message), "/register").await
        }
    }
}

async fn store_logout<B: FortifyBackend>(
    mut auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    if !logout(&mut auth).await? {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    }
    Ok(request.redirect(Inertia::redirect("/"))?)
}

async fn dashboard<F: FortifyFrontend, B: FortifyBackend>(
    State(frontend): State<F>,
    auth: AuthSession<B>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !user.email_is_verified() {
        return Ok(request.redirect(Inertia::redirect("/email/verify"))?);
    }
    let csrf_token = get_or_create_token(&auth.session).await.map_err(FortifyHttpError::Csrf)?;
    Ok(request.render(
        Inertia::response(
            frontend.fortify_components().dashboard,
            DashboardProps { auth: DashboardAuth { user: user.public() }, csrf_token },
        ),
        move |context| frontend.fortify_document(context),
    )?)
}

fn registration_validation_errors(validation: &ValidatorInputErrors) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    if validation.field_errors().contains_key("name") {
        errors.insert("name".into(), "The name must be between 1 and 255 characters.".into());
    }
    if validation.field_errors().contains_key("email") {
        errors.insert("email".into(), "The email must be a valid email address.".into());
    }
    if validation.field_errors().contains_key("password") {
        errors.insert("password".into(), "The password must be at least 8 characters.".into());
    }
    errors
}
