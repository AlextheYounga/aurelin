use aurelin_framework::{
    axum::{
        Json, Router,
        extract::{ConnectInfo, Path, rejection::JsonRejection},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::{delete, get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::{AuthError, AuthSession},
    frontend::Frontend,
    routes::{
        login_session::set_login_expiry,
        password_confirmation::{
            PASSWORD_CONFIRMATION_REDIRECT, PASSWORD_CONFIRMED_AT, password_is_confirmed,
        },
        web_error::WebError,
    },
};

mod rate_limit;

const REGISTRATION_CEREMONY: &str = "passkeys.registration";
const LOGIN_CEREMONY: &str = "passkeys.login";
const CONFIRMATION_CEREMONY: &str = "passkeys.confirmation";

#[derive(Deserialize)]
struct RegistrationInput {
    name: String,
    credential: Value,
}

#[derive(Deserialize)]
struct VerificationInput {
    credential: Value,
    #[serde(default)]
    remember: bool,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/.well-known/passkey-endpoints", get(passkey_endpoints))
        .route("/passkeys/login/options", get(login_options))
        .route("/passkeys/login", post(login))
        .route("/passkeys/confirm/options", get(confirmation_options))
        .route("/passkeys/confirm", post(confirm))
        .route("/user/passkeys/options", get(registration_options))
        .route("/user/passkeys", post(register))
        .route("/user/passkeys/{id}", delete(destroy))
}

async fn passkey_endpoints(auth: AuthSession) -> Json<Value> {
    let security = format!("{}/settings/security", auth.backend.application_url());
    Json(json!({
        "enroll": security.clone(),
        "manage": security,
    }))
}

async fn registration_options(
    auth: AuthSession,
    ConnectInfo(address): ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response());
    };
    if !password_is_confirmed(&auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/security")
            .await?;
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Password confirmation required." })),
        )
            .into_response());
    }
    let limit = match rate_limit::attempt(&auth, address, None).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let ceremony = auth.backend.start_passkey_registration(user).await?;
    auth.session
        .insert(REGISTRATION_CEREMONY, ceremony.id)
        .await?;
    Ok(limit.add_headers(
        Json(json!({ "options": public_key_options(ceremony.options) })).into_response(),
    ))
}

async fn register(
    auth: AuthSession,
    ConnectInfo(address): ConnectInfo<std::net::SocketAddr>,
    input: Result<Json<RegistrationInput>, JsonRejection>,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response());
    };
    if !password_is_confirmed(&auth).await? {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Password confirmation required." })),
        )
            .into_response());
    }
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    let limit = match rate_limit::attempt(&auth, address, Some(&input.credential)).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let Some(ceremony_id) = auth.session.remove::<String>(REGISTRATION_CEREMONY).await? else {
        return Ok(limit.add_headers(invalid_ceremony()));
    };
    let passkey = match auth
        .backend
        .finish_passkey_registration(&ceremony_id, user, input.name, input.credential)
        .await
    {
        Ok(passkey) => passkey,
        Err(AuthError::PasskeyAlreadyRegistered) => {
            return Ok(limit.add_headers(
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "message": "This passkey is already registered." })),
                )
                    .into_response(),
            ));
        }
        Err(AuthError::Passkey(_)) => return Ok(limit.add_headers(invalid_passkey())),
        Err(error) => return Err(error.into()),
    };
    Ok(limit.add_headers(
        Json(json!({ "id": passkey.id.to_string(), "name": passkey.name })).into_response(),
    ))
}

async fn login_options(
    auth: AuthSession,
    ConnectInfo(address): ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({ "message": "Already authenticated." })),
        )
            .into_response());
    }
    let limit = match rate_limit::attempt(&auth, address, None).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let ceremony = auth.backend.start_passkey_login()?;
    auth.session.insert(LOGIN_CEREMONY, ceremony.id).await?;
    Ok(limit.add_headers(
        Json(json!({ "options": public_key_options(ceremony.options) })).into_response(),
    ))
}

async fn login(
    mut auth: AuthSession,
    ConnectInfo(address): ConnectInfo<std::net::SocketAddr>,
    input: Result<Json<VerificationInput>, JsonRejection>,
) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({ "message": "Already authenticated." })),
        )
            .into_response());
    }
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    let limit = match rate_limit::attempt(&auth, address, Some(&input.credential)).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let remember = input.remember;
    let Some(ceremony_id) = auth.session.remove::<String>(LOGIN_CEREMONY).await? else {
        return Ok(limit.add_headers(invalid_ceremony()));
    };
    let user = match auth
        .backend
        .finish_passkey_login(&ceremony_id, input.credential)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) | Err(AuthError::Passkey(_)) => return Ok(limit.add_headers(invalid_passkey())),
        Err(error) => return Err(error.into()),
    };
    auth.login(&user).await?;
    set_login_expiry(&auth.session, remember);
    Ok(limit.add_headers(Json(json!({ "redirect": "/dashboard" })).into_response()))
}

async fn confirmation_options(
    auth: AuthSession,
    ConnectInfo(address): ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response());
    };
    let limit = match rate_limit::attempt(&auth, address, None).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let ceremony = auth.backend.start_passkey_confirmation(user).await?;
    auth.session
        .insert(CONFIRMATION_CEREMONY, ceremony.id)
        .await?;
    Ok(limit.add_headers(
        Json(json!({ "options": public_key_options(ceremony.options) })).into_response(),
    ))
}

async fn confirm(
    auth: AuthSession,
    ConnectInfo(address): ConnectInfo<std::net::SocketAddr>,
    input: Result<Json<VerificationInput>, JsonRejection>,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response());
    };
    let Json(input) = match input {
        Ok(input) => input,
        Err(error) => return Ok(error.into_response()),
    };
    let limit = match rate_limit::attempt(&auth, address, Some(&input.credential)).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let Some(ceremony_id) = auth.session.remove::<String>(CONFIRMATION_CEREMONY).await? else {
        return Ok(limit.add_headers(invalid_ceremony()));
    };
    let confirmed = match auth
        .backend
        .finish_passkey_confirmation(&ceremony_id, user, input.credential)
        .await
    {
        Ok(confirmed) => confirmed,
        Err(AuthError::Passkey(_)) => false,
        Err(error) => return Err(error.into()),
    };
    if !confirmed {
        return Ok(limit.add_headers(invalid_passkey()));
    }
    auth.session
        .insert(PASSWORD_CONFIRMED_AT, unix_timestamp()?)
        .await?;
    Ok(limit.add_headers(Json(json!({ "redirect": "/settings/security" })).into_response()))
}

async fn destroy(
    auth: AuthSession,
    Path(id): Path<i64>,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !password_is_confirmed(&auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/security")
            .await?;
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    }
    auth.backend.delete_passkey(user.id, id).await?;
    Ok(request.redirect(Inertia::redirect("/settings/security"))?)
}

fn public_key_options(mut options: Value) -> Value {
    options
        .get_mut("publicKey")
        .map(Value::take)
        .unwrap_or(options)
}

fn invalid_ceremony() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "message": "The passkey ceremony expired." })),
    )
        .into_response()
}

fn invalid_passkey() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "message": "The passkey could not be verified." })),
    )
        .into_response()
}

fn unix_timestamp() -> Result<u64, std::time::SystemTimeError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}
