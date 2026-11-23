//! Aurelin request protections for Leptos server functions.

use axum_tower_sessions_csrf::get_or_create_token;
use thiserror::Error;
use tower_sessions::Session;

/// Failures while issuing or validating a synchronizer token in a server function.
#[derive(Debug, Error)]
pub enum CsrfError {
    #[error("Leptos request context is unavailable: {0}")]
    RequestContext(String),
    #[error("CSRF session operation failed: {0}")]
    Session(String),
    #[error("invalid or missing CSRF token")]
    InvalidToken,
}

/// Returns the synchronizer token associated with the current Leptos request session.
pub async fn csrf_token() -> Result<String, CsrfError> {
    let session: Session =
        leptos_axum::extract().await.map_err(|error| CsrfError::RequestContext(error.to_string()))?;
    get_or_create_token(&session).await.map_err(CsrfError::Session)
}

/// Validates a state-changing server function's synchronizer token.
pub async fn validate_csrf_token(submitted_token: &str) -> Result<(), CsrfError> {
    let expected_token = csrf_token().await?;
    if constant_time_eq::constant_time_eq(submitted_token.as_bytes(), expected_token.as_bytes()) {
        return Ok(());
    }
    Err(CsrfError::InvalidToken)
}
