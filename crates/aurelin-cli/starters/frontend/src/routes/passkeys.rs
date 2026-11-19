use aurelin_framework::{
    axum::{
        Json, Router,
        extract::{Path, State, rejection::JsonRejection},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::{delete, get, post},
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    auth::{AuthError, AuthSession},
    frontend::Frontend,
    frontend_config::PASSKEY_SETTINGS_COMPONENT,
    routes::{
        password_confirmation::{
            PASSWORD_CONFIRMATION_REDIRECT, PASSWORD_CONFIRMED_AT, password_is_confirmed,
        },
        web_error::WebError,
    },
};

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

#[derive(Serialize)]
struct PasskeyProps {
    passkeys: Vec<PasskeyItem>,
    csrf_token: String,
}

#[derive(Serialize)]
struct PasskeyItem {
    id: i64,
    name: String,
    last_used_at: Option<i64>,
    created_at: i64,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/settings/passkeys", get(settings))
        .route("/passkeys/login/options", get(login_options))
        .route("/passkeys/login", post(login))
        .route("/passkeys/confirm/options", get(confirmation_options))
        .route("/passkeys/confirm", post(confirm))
        .route("/user/passkeys/options", get(registration_options))
        .route("/user/passkeys", post(register))
        .route("/user/passkeys/{id}", delete(destroy))
}

async fn settings(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok(request.redirect(Inertia::redirect("/login"))?);
    };
    if !password_is_confirmed(&auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/passkeys")
            .await?;
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let passkeys = auth
        .backend
        .passkeys_for_user(user.id)
        .await?
        .into_iter()
        .map(|passkey| PasskeyItem {
            id: passkey.id,
            name: passkey.name,
            last_used_at: passkey.last_used_at,
            created_at: passkey.created_at,
        })
        .collect();
    Ok(request.render(
        Inertia::response(
            PASSKEY_SETTINGS_COMPONENT,
            PasskeyProps {
                passkeys,
                csrf_token,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn registration_options(auth: AuthSession) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response());
    };
    if !password_is_confirmed(&auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/passkeys")
            .await?;
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Password confirmation required." })),
        )
            .into_response());
    }
    let ceremony = auth.backend.start_passkey_registration(user).await?;
    auth.session
        .insert(REGISTRATION_CEREMONY, ceremony.id)
        .await?;
    Ok(Json(json!({ "options": public_key_options(ceremony.options) })).into_response())
}

async fn register(
    auth: AuthSession,
    input: Result<Json<RegistrationInput>, JsonRejection>,
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
    let Some(ceremony_id) = auth.session.remove::<String>(REGISTRATION_CEREMONY).await? else {
        return Ok(invalid_ceremony());
    };
    let passkey = match auth
        .backend
        .finish_passkey_registration(&ceremony_id, user, input.name, input.credential)
        .await
    {
        Ok(passkey) => passkey,
        Err(AuthError::PasskeyAlreadyRegistered) => {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "message": "This passkey is already registered." })),
            )
                .into_response());
        }
        Err(AuthError::Passkey(_)) => return Ok(invalid_passkey()),
        Err(error) => return Err(error.into()),
    };
    Ok(Json(json!({ "id": passkey.id.to_string(), "name": passkey.name })).into_response())
}

async fn login_options(auth: AuthSession) -> Result<Response, WebError> {
    if auth.user.is_some() {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({ "message": "Already authenticated." })),
        )
            .into_response());
    }
    let ceremony = auth.backend.start_passkey_login()?;
    auth.session.insert(LOGIN_CEREMONY, ceremony.id).await?;
    Ok(Json(json!({ "options": public_key_options(ceremony.options) })).into_response())
}

async fn login(
    mut auth: AuthSession,
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
    let _remember = input.remember;
    let Some(ceremony_id) = auth.session.remove::<String>(LOGIN_CEREMONY).await? else {
        return Ok(invalid_ceremony());
    };
    let user = match auth
        .backend
        .finish_passkey_login(&ceremony_id, input.credential)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) | Err(AuthError::Passkey(_)) => return Ok(invalid_passkey()),
        Err(error) => return Err(error.into()),
    };
    auth.login(&user).await?;
    Ok(Json(json!({ "redirect": "/dashboard" })).into_response())
}

async fn confirmation_options(auth: AuthSession) -> Result<Response, WebError> {
    let Some(user) = auth.user.as_ref() else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response());
    };
    let ceremony = auth.backend.start_passkey_confirmation(user).await?;
    auth.session
        .insert(CONFIRMATION_CEREMONY, ceremony.id)
        .await?;
    Ok(Json(json!({ "options": public_key_options(ceremony.options) })).into_response())
}

async fn confirm(
    auth: AuthSession,
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
    let Some(ceremony_id) = auth.session.remove::<String>(CONFIRMATION_CEREMONY).await? else {
        return Ok(invalid_ceremony());
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
        return Ok(invalid_passkey());
    }
    auth.session
        .insert(PASSWORD_CONFIRMED_AT, unix_timestamp()?)
        .await?;
    Ok(Json(json!({ "redirect": "/settings/passkeys" })).into_response())
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
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/passkeys")
            .await?;
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    }
    auth.backend.delete_passkey(user.id, id).await?;
    Ok(request.redirect(Inertia::redirect("/settings/passkeys"))?)
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
