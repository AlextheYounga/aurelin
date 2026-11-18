use std::{collections::BTreeMap, net::SocketAddr};

use axum_tower_sessions_csrf::get_or_create_token;
use kiln_framework::{
    axum::{
        Json, Router,
        extract::{ConnectInfo, Path, Query, State},
        response::Response,
        routing::{get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    auth::{AuthSession, CreateUser, Credentials, PublicUser},
    frontend::Frontend,
    frontend_config::{
        DASHBOARD_COMPONENT, FORGOT_PASSWORD_COMPONENT, LOGIN_COMPONENT, REGISTER_COMPONENT,
        RESET_PASSWORD_COMPONENT,
    },
    routes::web_error::WebError,
};

const FLASH_ERRORS: &str = "auth.errors";
const FLASH_STATUS: &str = "auth.status";

type ValidationErrors = BTreeMap<String, String>;

#[derive(Serialize)]
struct AuthPageProps {
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

#[derive(Deserialize, Validate)]
struct ForgotPasswordInput {
    #[validate(email, length(max = 255))]
    email: String,
}

#[derive(Deserialize, Validate)]
struct ResetPasswordInput {
    #[validate(email, length(max = 255))]
    email: String,
    token: String,
    #[validate(length(min = 8, max = 1024))]
    password: String,
    password_confirmation: String,
}

#[derive(Deserialize)]
struct ResetPasswordQuery {
    email: String,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/dashboard", get(dashboard))
        .route("/login", get(login).post(store_login))
        .route("/register", get(register).post(store_registration))
        .route(
            "/forgot-password",
            get(forgot_password).post(send_password_reset_link),
        )
        .route("/reset-password/{token}", get(reset_password))
        .route("/reset-password", post(store_reset_password))
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

async fn forgot_password(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    render_auth_page(frontend, auth, request, FORGOT_PASSWORD_COMPONENT).await
}

async fn send_password_reset_link(
    auth: AuthSession,
    request: InertiaRequest,
    Json(input): Json<ForgotPasswordInput>,
) -> Result<Response, WebError> {
    if input.validate().is_err() {
        return redirect_with_error(
            &auth,
            &request,
            "email",
            "The email must be a valid email address.",
            "/forgot-password",
        )
        .await;
    }
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
    Query(query): Query<ResetPasswordQuery>,
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
    Json(input): Json<ResetPasswordInput>,
) -> Result<Response, WebError> {
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
    if !auth
        .backend
        .reset_password(&input.email, &input.token, &input.password)
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

async fn store_login(
    mut auth: AuthSession,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    request: InertiaRequest,
    Json(input): Json<LoginInput>,
) -> Result<Response, WebError> {
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
    if input.validate().is_err() {
        auth.backend.record_failed_login(key).await;
        return invalid_credentials(&auth, &request).await;
    }
    let user = auth
        .authenticate(Credentials {
            email: input.email,
            password: input.password,
        })
        .await?;
    let Some(user) = user else {
        auth.backend.record_failed_login(key).await;
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
    Json(input): Json<RegistrationInput>,
) -> Result<Response, WebError> {
    let errors = registration_errors(&input);
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

fn registration_errors(input: &RegistrationInput) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    if let Err(validation) = input.validate() {
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
    }
    if input.password != input.password_confirmation {
        errors.insert(
            "password".into(),
            "The password confirmation does not match.".into(),
        );
    }
    errors
}

async fn redirect_with_error(
    auth: &AuthSession,
    request: &InertiaRequest,
    field: &str,
    message: &str,
    destination: &str,
) -> Result<Response, WebError> {
    let mut errors = ValidationErrors::new();
    errors.insert(field.into(), message.into());
    auth.session.insert(FLASH_ERRORS, errors).await?;
    Ok(request.redirect(Inertia::redirect(destination))?)
}
