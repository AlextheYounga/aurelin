use aurelin_framework::{
    axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    inertia::axum::InertiaError,
};
use thiserror::Error;
use tower_sessions::session;

use crate::auth::{AuthBackend, AuthError};

#[derive(Debug, Error)]
pub(super) enum WebError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    AuthSession(#[from] axum_login::Error<AuthBackend>),
    #[error("CSRF session operation failed: {0}")]
    Csrf(String),
    #[error(transparent)]
    Inertia(#[from] InertiaError),
    #[error(transparent)]
    Session(#[from] session::Error),
    #[error("system clock is before the Unix epoch")]
    Clock(#[from] std::time::SystemTimeError),
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self, "request failed");
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    }
}
