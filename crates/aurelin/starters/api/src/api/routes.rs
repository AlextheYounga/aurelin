use std::time::Duration;

use aurelin_framework::{
    auth::{FixedWindowRateLimiter, RateLimitOutcome},
    axum::{
        Extension, Json, Router,
        extract::Path,
        http::StatusCode,
        middleware,
        routing::{delete, get},
    },
};
use aurelin_sanctum::{
    Abilities, ApiError, AuthenticatedToken, IssueToken, ServiceError, TokenService,
    authenticate_bearer,
};
use sea_orm::EntityTrait;
use serde_json::{Value, json};

use super::{Repository, validation::IssueRequest};
use crate::{
    auth::{AuthSession, PublicUser},
    entities::user,
    frontend::Frontend,
};

type Service = TokenService<Repository>;

pub fn browser_routes(service: Service) -> Router<Frontend> {
    Router::new()
        .route("/tokens", get(list).post(issue))
        .route("/tokens/{id}", delete(revoke))
        .layer(Extension(FixedWindowRateLimiter::new(
            6,
            Duration::from_secs(60),
        )))
        .layer(Extension(service))
}

pub fn bearer_routes(service: Service, database: sea_orm::DatabaseConnection) -> Router {
    Router::new()
        .route("/api/user", get(current_user))
        .route("/api/token", delete(revoke_current))
        .layer(Extension(database))
        .layer(Extension(service.clone()))
        .layer(middleware::from_fn_with_state(
            service,
            authenticate_bearer::<Repository>,
        ))
}

async fn issue(
    auth: AuthSession,
    Extension(service): Extension<Service>,
    Extension(limiter): Extension<FixedWindowRateLimiter>,
    Json(input): Json<IssueRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = auth.user.ok_or(ApiError::Unauthenticated)?;
    if matches!(
        limiter.attempt(&user.id.to_string()).await,
        RateLimitOutcome::Limited { .. }
    ) {
        return Ok((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"code": "rate_limited"})),
        ));
    }
    if !input.is_valid() {
        return Ok(super::validation::invalid_request());
    }
    let abilities = Abilities::new(input.abilities);
    let issued = service
        .issue(IssueToken {
            owner: &user.id,
            name: &input.name,
            abilities: &abilities,
            expires_at: input.expires_at,
            allow_wildcard: false,
        })
        .await
        .map_err(service_error)?;
    let id = issued.id;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id": id, "token": issued.into_plaintext()})),
    ))
}

async fn list(
    auth: AuthSession,
    Extension(service): Extension<Service>,
) -> Result<Json<Value>, ApiError> {
    let user = auth.user.ok_or(ApiError::Unauthenticated)?;
    let tokens = service.list(&user.id).await.map_err(service_error)?;
    Ok(Json(json!(tokens.into_iter().map(|t| json!({"id": t.id, "name": t.name, "abilities": t.abilities.as_slice(), "expires_at": t.expires_at, "last_used_at": t.last_used_at})).collect::<Vec<_>>())))
}

async fn revoke(
    auth: AuthSession,
    Extension(service): Extension<Service>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = auth.user.ok_or(ApiError::Unauthenticated)?;
    if service.revoke(&user.id, id).await.map_err(service_error)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::Forbidden)
    }
}

async fn current_user(
    Extension(identity): Extension<AuthenticatedToken<i64>>,
    Extension(database): Extension<sea_orm::DatabaseConnection>,
) -> Result<Json<PublicUser>, ApiError> {
    let user = user::Entity::find_by_id(identity.owner)
        .one(&database)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::Unauthenticated)?;
    Ok(Json(PublicUser::from(&user)))
}

async fn revoke_current(
    Extension(identity): Extension<AuthenticatedToken<i64>>,
    Extension(service): Extension<Service>,
) -> Result<StatusCode, ApiError> {
    if service
        .revoke_current(&identity)
        .await
        .map_err(service_error)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::Unauthenticated)
    }
}

fn service_error(error: ServiceError<sea_orm::DbErr>) -> ApiError {
    match error {
        ServiceError::Unauthenticated => ApiError::Unauthenticated,
        ServiceError::WildcardNotAllowed => ApiError::Forbidden,
        ServiceError::Repository(_) => ApiError::Internal,
    }
}
