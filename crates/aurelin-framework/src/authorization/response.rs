use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// The result of evaluating an authorization ability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationResponse {
    allowed: bool,
    message: Option<String>,
    code: Option<String>,
    status: Option<StatusCode>,
}

impl AuthorizationResponse {
    /// Allow the requested action.
    pub fn allow() -> Self {
        Self { allowed: true, message: None, code: None, status: None }
    }

    /// Deny the requested action with the default HTTP status.
    pub fn deny() -> Self {
        Self { allowed: false, message: None, code: None, status: None }
    }

    /// Deny the requested action with a particular HTTP status.
    pub fn deny_with_status(status: StatusCode) -> Self {
        Self::deny().with_status(status)
    }

    /// Conceal a denied resource behind a not-found response.
    pub fn deny_as_not_found() -> Self {
        Self::deny_with_status(StatusCode::NOT_FOUND)
    }

    /// Attach a user-facing denial message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Attach an application-defined denial reason code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Set the HTTP status used when this response is denied.
    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.status = Some(status);
        self
    }

    /// Whether the action was allowed.
    pub fn allowed(&self) -> bool {
        self.allowed
    }

    /// Whether the action was denied.
    pub fn denied(&self) -> bool {
        !self.allowed
    }

    /// The optional user-facing message.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The optional application-defined reason code.
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// The explicitly selected HTTP status, if any.
    pub fn status(&self) -> Option<StatusCode> {
        self.status
    }

    /// Return an error when this response denies the action.
    pub fn authorize(self) -> Result<Self, AuthorizationError> {
        if self.allowed() { Ok(self) } else { Err(AuthorizationError::Denied(self)) }
    }
}

/// Convert a policy result into an authorization response.
pub trait IntoAuthorizationResponse {
    fn into_authorization_response(self) -> AuthorizationResponse;

    #[doc(hidden)]
    fn into_on_demand_response(self, _allow_when: bool) -> AuthorizationResponse
    where
        Self: Sized,
    {
        self.into_authorization_response()
    }
}

impl IntoAuthorizationResponse for AuthorizationResponse {
    fn into_authorization_response(self) -> AuthorizationResponse {
        self
    }
}

impl IntoAuthorizationResponse for bool {
    fn into_authorization_response(self) -> AuthorizationResponse {
        if self { AuthorizationResponse::allow() } else { AuthorizationResponse::deny() }
    }

    fn into_on_demand_response(self, allow_when: bool) -> AuthorizationResponse {
        (self == allow_when).into_authorization_response()
    }
}

/// An authorization denial or invalid gate invocation.
#[derive(Debug, thiserror::Error)]
pub enum AuthorizationError {
    /// The gate or policy denied the action.
    #[error("{message}", message = denial_message(.0))]
    Denied(AuthorizationResponse),
    /// The ability was invoked with the wrong context type.
    #[error(transparent)]
    Gate(#[from] super::GateError),
}

impl AuthorizationError {
    /// The authorization response associated with a denial.
    pub fn response(&self) -> Option<&AuthorizationResponse> {
        match self {
            Self::Denied(response) => Some(response),
            Self::Gate(_) => None,
        }
    }
}

impl IntoResponse for AuthorizationError {
    fn into_response(self) -> Response {
        match self {
            Self::Denied(response) => {
                let status = response.status.unwrap_or(StatusCode::FORBIDDEN);
                let message = denial_message(&response).to_owned();
                (status, message).into_response()
            }
            Self::Gate(error) => {
                tracing::error!(error = %error, "authorization gate invocation failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    }
}

fn denial_message(response: &AuthorizationResponse) -> &str {
    response.message.as_deref().unwrap_or("This action is unauthorized.")
}
