use std::{
    net::SocketAddr,
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

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
use axum_login::AuthUser;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    AuthSession, FortifyBackend, FortifyFrontend, FortifyHttpError, PasskeyLoginOutcome, PasskeyRegistrationOutcome,
    PasskeyRegistrationRequest,
    events::{self, Login},
    login_session::set_login_expiry,
    password_confirmation::{PASSWORD_CONFIRMATION_REDIRECT, PASSWORD_CONFIRMED_AT, password_is_confirmed},
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

pub fn routes<F, B>() -> Router<F>
where
    F: FortifyFrontend,
    B: FortifyBackend,
{
    Router::new()
        .route("/.well-known/passkey-endpoints", get(passkey_endpoints::<B>))
        .route("/passkeys/login/options", get(login_options::<B>))
        .route("/passkeys/login", post(login::<B>))
        .route("/passkeys/confirm/options", get(confirmation_options::<B>))
        .route("/passkeys/confirm", post(confirm::<B>))
        .route("/user/passkeys/options", get(registration_options::<B>))
        .route("/user/passkeys", post(register::<B>))
        .route("/user/passkeys/{id}", delete(destroy::<B>))
}

async fn passkey_endpoints<B: FortifyBackend>(auth: AuthSession<B>) -> Json<Value> {
    let security = format!("{}/settings/security", auth.backend.application_url());
    Json(json!({
        "enroll": &security,
        "manage": &security,
    }))
}

async fn registration_options<B: FortifyBackend>(
    auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((StatusCode::UNAUTHORIZED, Json(json!({ "message": "Unauthenticated." }))).into_response());
    };
    if !password_is_confirmed(&auth).await? {
        auth.session.insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/security").await?;
        return Ok(
            (StatusCode::FORBIDDEN, Json(json!({ "message": "Password confirmation required." }))).into_response()
        );
    }
    let limit = match rate_limit::attempt(&auth, address, None).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let ceremony = auth.backend.start_passkey_registration(user).await.map_err(FortifyHttpError::backend)?;
    auth.session.insert(REGISTRATION_CEREMONY, ceremony.id).await?;
    Ok(limit.add_headers(Json(json!({ "options": public_key_options(ceremony.options) })).into_response()))
}

async fn register<B: FortifyBackend>(
    auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    input: Result<Json<RegistrationInput>, JsonRejection>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((StatusCode::UNAUTHORIZED, Json(json!({ "message": "Unauthenticated." }))).into_response());
    };
    if !password_is_confirmed(&auth).await? {
        return Ok(
            (StatusCode::FORBIDDEN, Json(json!({ "message": "Password confirmation required." }))).into_response()
        );
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
    let outcome = auth
        .backend
        .finish_passkey_registration(PasskeyRegistrationRequest {
            ceremony_id: &ceremony_id,
            user,
            name: input.name,
            credential: input.credential,
        })
        .await
        .map_err(FortifyHttpError::backend)?;
    let (id, name) = match outcome {
        PasskeyRegistrationOutcome::Registered { id, name } => (id, name),
        PasskeyRegistrationOutcome::AlreadyRegistered => {
            return Ok(limit.add_headers(
                (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "message": "This passkey is already registered." })))
                    .into_response(),
            ));
        }
        PasskeyRegistrationOutcome::Invalid => return Ok(limit.add_headers(invalid_passkey())),
    };
    Ok(limit.add_headers(Json(json!({ "id": id, "name": name })).into_response()))
}

async fn login_options<B: FortifyBackend>(
    auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok((StatusCode::CONFLICT, Json(json!({ "message": "Already authenticated." }))).into_response());
    }
    let limit = match rate_limit::attempt(&auth, address, None).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let ceremony = auth.backend.start_passkey_login().map_err(FortifyHttpError::backend)?;
    auth.session.insert(LOGIN_CEREMONY, ceremony.id).await?;
    Ok(limit.add_headers(Json(json!({ "options": public_key_options(ceremony.options) })).into_response()))
}

async fn login<B: FortifyBackend>(
    mut auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    input: Result<Json<VerificationInput>, JsonRejection>,
) -> Result<Response, FortifyHttpError> {
    if auth.user.is_some() {
        return Ok((StatusCode::CONFLICT, Json(json!({ "message": "Already authenticated." }))).into_response());
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
        .map_err(FortifyHttpError::backend)?
    {
        PasskeyLoginOutcome::Authenticated(user) => user,
        PasskeyLoginOutcome::Invalid => return Ok(limit.add_headers(invalid_passkey())),
    };
    auth.login(&user).await.map_err(FortifyHttpError::auth_session)?;
    set_login_expiry(&auth.session, remember);
    events::observe(auth.backend.authentication_events(), Login { user_id: user.id(), remember }).await;
    Ok(limit.add_headers(Json(json!({ "redirect": "/dashboard" })).into_response()))
}

async fn confirmation_options<B: FortifyBackend>(
    auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((StatusCode::UNAUTHORIZED, Json(json!({ "message": "Unauthenticated." }))).into_response());
    };
    let limit = match rate_limit::attempt(&auth, address, None).await {
        Ok(limit) => limit,
        Err(response) => return Ok(response),
    };
    let ceremony = auth.backend.start_passkey_confirmation(user).await.map_err(FortifyHttpError::backend)?;
    auth.session.insert(CONFIRMATION_CEREMONY, ceremony.id).await?;
    Ok(limit.add_headers(Json(json!({ "options": public_key_options(ceremony.options) })).into_response()))
}

async fn confirm<B: FortifyBackend>(
    auth: AuthSession<B>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    input: Result<Json<VerificationInput>, JsonRejection>,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((StatusCode::UNAUTHORIZED, Json(json!({ "message": "Unauthenticated." }))).into_response());
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
    let confirmed = auth
        .backend
        .finish_passkey_confirmation(&ceremony_id, user, input.credential)
        .await
        .map_err(FortifyHttpError::backend)?;
    if !confirmed {
        return Ok(limit.add_headers(invalid_passkey()));
    }
    auth.session.insert(PASSWORD_CONFIRMED_AT, unix_timestamp()?).await?;
    Ok(limit.add_headers(Json(json!({ "redirect": "/settings/security" })).into_response()))
}

async fn destroy<B: FortifyBackend>(
    auth: AuthSession<B>,
    Path(id): Path<i64>,
    request: InertiaRequest,
) -> Result<Response, FortifyHttpError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !password_is_confirmed(&auth).await? {
        auth.session.insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/security").await?;
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    }
    auth.backend.delete_passkey(user.id(), id).await.map_err(FortifyHttpError::backend)?;
    Ok(request.redirect(Inertia::redirect("/settings/security"))?)
}

fn public_key_options(mut options: Value) -> Value {
    options.get_mut("publicKey").map(Value::take).unwrap_or(options)
}

fn invalid_ceremony() -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "message": "The passkey ceremony expired." }))).into_response()
}

fn invalid_passkey() -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "message": "The passkey could not be verified." }))).into_response()
}

fn unix_timestamp() -> Result<u64, SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
