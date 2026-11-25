use aurelin_framework::{
    axum::{Router, extract::State, response::Response, routing::get},
    events::EventDispatcher,
    health,
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use serde::Serialize;

use crate::{
    auth::{AuthSession, PublicUser},
    config::frontend::WELCOME_COMPONENT,
    frontend::Frontend,
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
        .route("/up", health::route(EventDispatcher::builder().build()))
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
