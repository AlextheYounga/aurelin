//! Browser and Inertia validation using request-scoped session flash data.
//!
//! Install [`flash_middleware`] inside the session layer, and pass page props
//! through [`validation_page_props`] before rendering. Use
//! [`BrowserValidated`] on form/JSON handlers, with authorization in an outer
//! route layer. API requests retain the JSON validation response.

use std::collections::BTreeMap;

use axum::{
    extract::{FromRequest, Request},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_valid::{HasValidate, ValidRejection};
use inertia_rs::axum::InertiaRequest;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use validator::Validate;

use super::{ValidationIssue, http::ValidationResponse, issues};

const FLASH_KEY: &str = "aurelin.validation.flash";

/// Explicit application-owned projection of non-sensitive, non-file form fields.
///
/// Never include credentials, passwords, tokens, secrets, files, or entire input
/// objects. The framework cannot determine whether an application-selected value
/// is sensitive; the caller is responsible for that security decision.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SafeOldInput(BTreeMap<String, String>);

impl SafeOldInput {
    /// Add one explicitly approved plain-text field.
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.insert(name.into(), value.into());
        self
    }
}

/// Implement on request data to select only fields safe to flash back to users.
pub trait OldInput {
    fn safe_old_input(&self) -> SafeOldInput;
}

/// Flash data is removed on the next request, regardless of its route.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ValidationFlash {
    issues: Vec<ValidationIssue>,
    bag: Option<String>,
    old: SafeOldInput,
}

impl ValidationFlash {
    /// First message per field, with an optional Inertia error-bag key.
    pub fn errors(&self) -> serde_json::Value {
        let mut fields = BTreeMap::new();
        for issue in &self.issues {
            fields.entry(issue.field.clone()).or_insert_with(|| issue.render_message());
        }
        match &self.bag {
            Some(bag) => serde_json::json!({ bag: fields }),
            None => serde_json::json!(fields),
        }
    }

    /// The explicitly selected old input, for application-owned form rendering.
    pub fn old_input(&self) -> &SafeOldInput {
        &self.old
    }
}

/// Read and remove validation flash once per request. Must run inside the session layer.
pub async fn flash_middleware(session: Session, mut request: Request, next: Next) -> Response {
    match session.remove::<ValidationFlash>(FLASH_KEY).await {
        Ok(flash) => {
            request.extensions_mut().insert(flash.unwrap_or_default());
            next.run(request).await
        }
        Err(error) => {
            tracing::error!(error = %error, "failed to read validation flash");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Add `errors` and `old` to route props before passing them to Inertia.
///
/// `inertia_rs` 0.4 inserts an empty `errors` prop before shared-prop merging,
/// so registering it as a `SharedProps` provider does not work.
pub fn validation_page_props(
    request: &InertiaRequest,
    mut props: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let flash = request.extension::<ValidationFlash>().cloned().unwrap_or_default();
    props.insert("errors".into(), flash.errors());
    props.insert("old".into(), serde_json::json!(flash.old));
    serde_json::Value::Object(props)
}

/// An Axum extractor that validates after any outer authorization middleware.
pub struct BrowserValidated<E>(pub E);

impl<S, E> FromRequest<S> for BrowserValidated<E>
where
    S: Send + Sync,
    E: HasValidate + FromRequest<S> + Send,
    E::Validate: Validate + OldInput,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let headers = request.headers().clone();
        let method = request.method().clone();
        let session = request.extensions().get::<Session>().cloned();
        let inner = E::from_request(request, state)
            .await
            .map_err(|error| ValidationResponse::from_rejection(ValidRejection::Inner(error)).into_response())?;
        let Err(errors) = inner.get_validate().validate() else {
            return Ok(Self(inner));
        };
        if wants_json(&headers) {
            return Err(
                ValidationResponse::from_rejection::<E::Rejection>(ValidRejection::Valid(errors)).into_response()
            );
        }
        let Some(session) = session else {
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
        };
        let bag = headers
            .get("x-inertia-error-bag")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .map(str::to_owned);
        let flash = ValidationFlash {
            issues: issues(&errors, bag.as_deref()),
            bag,
            old: inner.get_validate().safe_old_input(),
        };
        if let Err(error) = session.insert(FLASH_KEY, flash).await {
            tracing::error!(error = %error, "failed to write validation flash");
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
        let location = redirect_back(&headers);
        let status = if method == Method::PUT
            || method == Method::PATCH
            || method == Method::DELETE
            || method.as_str() == "QUERY"
        {
            StatusCode::SEE_OTHER
        } else {
            StatusCode::FOUND
        };
        Err((status, [(header::LOCATION, location), (header::VARY, "X-Inertia".to_owned())]).into_response())
    }
}

fn wants_json(headers: &HeaderMap) -> bool {
    if headers.contains_key("x-inertia") {
        return false;
    }
    [header::ACCEPT, header::CONTENT_TYPE].iter().any(|name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|value| value.contains("/json") || value.contains("+json"))
    })
}

fn redirect_back(headers: &HeaderMap) -> String {
    let Some(uri) = headers.get(header::REFERER).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<Uri>().ok())
    else {
        return "/".into();
    };
    if let Some(authority) = uri.authority() {
        let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
        if host != Some(authority.as_str()) {
            return "/".into();
        }
    }
    uri.path_and_query()
        .filter(|path| path.as_str().starts_with('/') && !path.as_str().starts_with("//"))
        .map_or_else(|| "/".into(), |path| path.as_str().to_owned())
}
