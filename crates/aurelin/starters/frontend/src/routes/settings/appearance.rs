use aurelin_framework::{
    axum::{extract::State, response::Response},
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use serde::Serialize;

use super::{SettingsAuth, unauthenticated_destination, verified_user};
use crate::{
    auth::{AuthSession, PublicUser},
    config::frontend::APPEARANCE_SETTINGS_COMPONENT,
    frontend::Frontend,
    routes::web_error::WebError,
};

#[derive(Serialize)]
struct AppearanceProps {
    auth: SettingsAuth,
    csrf_token: String,
}

pub(super) async fn show(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = verified_user(&auth) else {
        return Ok(request.redirect(Inertia::redirect(unauthenticated_destination(&auth)))?);
    };
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    Ok(request.render(
        Inertia::response(
            APPEARANCE_SETTINGS_COMPONENT,
            AppearanceProps {
                auth: SettingsAuth {
                    user: PublicUser::from(user),
                },
                csrf_token,
            },
        ),
        move |context| frontend.document(context),
    )?)
}
