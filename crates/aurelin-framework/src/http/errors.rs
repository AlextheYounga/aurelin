//! Explicit, safe JSON application error responses.
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::{error::Error, fmt};

/// Public HTTP failures. Internal sources remain available through `Error::source`.
#[derive(Debug)]
pub enum AppError {
    NotFound,
    MethodNotAllowed,
    Unauthorized,
    Forbidden,
    Internal(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.public().2)
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Internal(source) => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl AppError {
    fn public(&self) -> (StatusCode, &'static str, &'static str) {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", "Not found."),
            Self::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", "Method not allowed."),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "Authentication required."),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "Forbidden."),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Server error."),
        }
    }
}

#[derive(Serialize)]
struct Body {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.public();
        (status, Json(Body { code, message })).into_response()
    }
}
