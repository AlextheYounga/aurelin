use std::collections::BTreeMap;

use kiln_framework::{
    axum::response::Response,
    inertia::{Inertia, axum::InertiaRequest},
};

use crate::{auth::AuthSession, routes::web_error::WebError};

pub(super) const FLASH_ERRORS: &str = "auth.errors";
pub(super) const FLASH_STATUS: &str = "auth.status";

pub(super) type ValidationErrors = BTreeMap<String, String>;

pub(super) async fn redirect_with_error(
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
