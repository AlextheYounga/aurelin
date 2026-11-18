use aurelin_framework::{
    axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, StatusCode, header::ACCEPT},
        response::{IntoResponse, Response},
        routing::get,
    },
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use serde::Serialize;

use crate::{
    auth::{AuthSession, PublicUser},
    frontend::Frontend,
    frontend_config::WELCOME_COMPONENT,
    routes::web_error::WebError,
};

#[derive(Serialize)]
struct WelcomeProps {
    message: &'static str,
    auth: AuthProps,
    csrf_token: String,
}

#[derive(Serialize)]
struct AuthProps {
    user: Option<PublicUser>,
}

pub fn routes() -> Router<Frontend> {
    Router::new()
        .route("/", get(welcome))
        .route("/up", get(health))
}

async fn welcome(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    Ok(request.render(
        Inertia::response(
            WELCOME_COMPONENT,
            WelcomeProps {
                message: "Welcome to Aurelin",
                auth: AuthProps {
                    user: auth.user.as_ref().map(PublicUser::from),
                },
                csrf_token,
            },
        ),
        move |context| frontend.document(context),
    )?)
}

async fn health(headers: HeaderMap) -> Response {
    let expects_json = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|mime| mime.trim().starts_with("application/json"))
        });
    if expects_json {
        return (StatusCode::OK, Json(serde_json::json!({ "status": "up" }))).into_response();
    }
    (StatusCode::OK, "Application up").into_response()
}
