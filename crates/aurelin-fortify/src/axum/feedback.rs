use std::collections::BTreeMap;

use aurelin_framework::{
    axum::response::Response,
    inertia::{Inertia, axum::InertiaRequest},
};

use super::{AuthSession, FortifyBackend, FortifyHttpError};

pub(super) const FLASH_ERRORS: &str = "auth.errors";
pub(super) const FLASH_STATUS: &str = "auth.status";

pub(super) type ValidationErrors = BTreeMap<String, String>;

pub(super) async fn redirect_with_error<B: FortifyBackend>(
    auth: &AuthSession<B>,
    request: &InertiaRequest,
    error: (&str, &str),
    destination: &str,
) -> Result<Response, FortifyHttpError> {
    let mut errors = ValidationErrors::new();
    errors.insert(error.0.into(), error.1.into());
    auth.session.insert(FLASH_ERRORS, errors).await?;
    Ok(request.redirect(Inertia::redirect(destination))?)
}
