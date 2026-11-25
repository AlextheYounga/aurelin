//! Safe JSON responses for validated Axum extractors.
//!
//! Use `Validated<Json<T>>`, `Validated<Form<T>>`, `Validated<Query<T>>`, or
//! `Validated<Path<T>>`. Authorization must run in an outer route layer or in
//! an earlier parts extractor; checks inside the handler run too late.
//! These extractors always return JSON, irrespective of Accept or X-Inertia.
//! They do not flash input or implement browser/Inertia redirects. For routes
//! requiring authorization, apply authorization as an outer Axum route layer;
//! authorizing in a handler after extraction does not protect validation.
//!
//! To customize a rejection, accept `Result<Validated<Json<T>>, ValidationResponse>`
//! in the handler and call `with_safe_response` on the error. Supply only a
//! message that is safe to expose publicly.

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Request},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_valid::{HasValidate, Valid, ValidRejection};
use serde::Serialize;
use std::collections::BTreeMap;
use validator::Validate;

use super::{ValidationIssue, issues};

/// An Axum extractor that rejects invalid inputs without invoking the handler.
pub struct Validated<E>(pub E);

impl<S, E> FromRequest<S> for Validated<E>
where
    S: Send + Sync,
    E: HasValidate + FromRequest<S>,
    E::Validate: Validate,
{
    type Rejection = ValidationResponse;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Valid::<E>::from_request(request, state)
            .await
            .map(|valid| Self(valid.0))
            .map_err(ValidationResponse::from_rejection)
    }
}

impl<S, E> FromRequestParts<S> for Validated<E>
where
    S: Send + Sync,
    E: HasValidate + FromRequestParts<S>,
    E::Validate: Validate,
{
    type Rejection = ValidationResponse;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Valid::<E>::from_request_parts(parts, state)
            .await
            .map(|valid| Self(valid.0))
            .map_err(ValidationResponse::from_rejection)
    }
}

/// A safe validation or extraction failure, customizable at an explicit handler boundary.
pub struct ValidationResponse {
    status: StatusCode,
    code: &'static str,
    message: String,
    issues: Vec<ValidationIssue>,
}

#[derive(Serialize)]
struct RenderedIssue<'a> {
    field: &'a str,
    code: &'a str,
    message: String,
    arguments: &'a BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bag: &'a Option<String>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    issues: Vec<RenderedIssue<'a>>,
}

impl ValidationResponse {
    pub(super) fn from_rejection<E: IntoResponse>(rejection: ValidRejection<E>) -> Self {
        match rejection {
            ValidRejection::Valid(errors) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "validation_failed",
                message: "The given data was invalid.".into(),
                issues: issues(&errors, None),
            },
            ValidRejection::Inner(error) => {
                let status = error.into_response().status();
                let status = match status {
                    StatusCode::UNSUPPORTED_MEDIA_TYPE => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
                    status if status.is_server_error() => StatusCode::INTERNAL_SERVER_ERROR,
                    _ => StatusCode::BAD_REQUEST,
                };
                Self {
                    status,
                    code: if status.is_server_error() { "internal_error" } else { "invalid_request" },
                    message: if status.is_server_error() { "Server error." } else { "The request could not be read." }
                        .into(),
                    issues: Vec::new(),
                }
            }
        }
    }

    /// Override status and message with application-owned, public-safe values.
    pub fn with_safe_response(mut self, status: StatusCode, message: impl Into<String>) -> Self {
        self.status = status;
        self.message = message.into();
        self
    }

    /// Replace issue messages with explicitly public-safe application text.
    pub fn with_safe_issue_messages(mut self, message: impl Fn(&ValidationIssue) -> Option<String>) -> Self {
        for issue in &mut self.issues {
            if let Some(text) = message(issue) {
                issue.descriptor = super::MessageDescriptor::Custom(text);
            }
        }
        self
    }
}

impl IntoResponse for ValidationResponse {
    fn into_response(self) -> Response {
        let rendered = self
            .issues
            .iter()
            .map(|issue| RenderedIssue {
                field: &issue.field,
                code: &issue.code,
                message: issue.render_message(),
                arguments: &issue.arguments,
                bag: &issue.bag,
            })
            .collect();
        (self.status, Json(ErrorBody { code: self.code, message: &self.message, issues: rendered })).into_response()
    }
}
