//! HTTP health diagnostics dispatched to application listeners.

use axum::{
    Json,
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::{IntoResponse, Response},
    routing::{MethodRouter, get},
};

use crate::events::EventDispatcher;

/// Dispatched when an HTTP health check is requested.
pub struct DiagnosingHealth;

/// Build a health route using the application's registered diagnostic listeners.
pub fn route<State>(events: EventDispatcher) -> MethodRouter<State>
where
    State: Clone + Send + Sync + 'static,
{
    get(move |headers: HeaderMap| {
        let events = events.clone();
        async move { respond(headers, events).await }
    })
}

async fn respond(headers: HeaderMap, events: EventDispatcher) -> Response {
    let healthy = events.dispatch(DiagnosingHealth).await.is_ok();
    if !healthy {
        tracing::error!("health diagnostic listener failed");
    }
    let status = if healthy { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR };
    let expects_json = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|mime| mime.trim().starts_with("application/json")));
    if expects_json {
        let state = if healthy { "up" } else { "down" };
        return (status, Json(serde_json::json!({ "status": state }))).into_response();
    }
    (status, if healthy { "Application up" } else { "Application down" }).into_response()
}
