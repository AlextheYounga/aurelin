use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use url::form_urlencoded;

use crate::{AuthenticatedToken, ServiceError, TokenRepository, TokenService};

#[derive(Clone, Copy, Debug)]
pub enum ApiError {
    Unauthenticated,
    Forbidden,
    Internal,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Unauthenticated => (StatusCode::UNAUTHORIZED, "unauthenticated"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "token_ability_denied"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        (status, Json(ErrorBody { code })).into_response()
    }
}

/// Authenticate a bearer-only route. Compose separately from cookie/CSRF routes;
/// never use the presence of an Authorization header to bypass CSRF.
pub async fn authenticate_bearer<R>(
    State(service): State<TokenService<R>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError>
where
    R: TokenRepository + 'static,
    R::Owner: Clone + Send + Sync + 'static,
{
    if request.uri().query().is_some_and(|query| {
        form_urlencoded::parse(query.as_bytes()).any(|(key, _)| key == "token" || key == "access_token")
    }) {
        return Err(ApiError::Unauthenticated);
    }
    let mut headers = request.headers().get_all(AUTHORIZATION).iter();
    let header = headers.next().ok_or(ApiError::Unauthenticated)?;
    if headers.next().is_some() {
        return Err(ApiError::Unauthenticated);
    }
    let header = header.to_str().map_err(|_| ApiError::Unauthenticated)?;
    let (scheme, value) = header.split_once(' ').ok_or(ApiError::Unauthenticated)?;
    if !scheme.eq_ignore_ascii_case("bearer") || value.is_empty() || value.contains(char::is_whitespace) {
        return Err(ApiError::Unauthenticated);
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ApiError::Internal)?;
    let now = i64::try_from(now.as_secs()).map_err(|_| ApiError::Internal)?;
    let identity = service.authenticate(value, now).await.map_err(|error| match error {
        ServiceError::Unauthenticated => ApiError::Unauthenticated,
        ServiceError::WildcardNotAllowed | ServiceError::Repository(_) => ApiError::Internal,
    })?;
    request.extensions_mut().insert(identity);
    Ok(next.run(request).await)
}

#[derive(Clone, Debug)]
pub enum AbilityRequirement {
    All(Vec<String>),
    Any(Vec<String>),
}

/// Abilities constrain a token; resource policies must still authorize the owner.
pub async fn require_abilities<Owner: Send + Sync + 'static>(
    State(requirement): State<AbilityRequirement>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = request.extensions().get::<AuthenticatedToken<Owner>>().ok_or(ApiError::Unauthenticated)?;
    let allowed = match requirement {
        AbilityRequirement::All(abilities) => abilities.iter().all(|ability| token.abilities.allows(ability)),
        AbilityRequirement::Any(abilities) => abilities.iter().any(|ability| token.abilities.allows(ability)),
    };
    if !allowed {
        return Err(ApiError::Forbidden);
    }
    Ok(next.run(request).await)
}
