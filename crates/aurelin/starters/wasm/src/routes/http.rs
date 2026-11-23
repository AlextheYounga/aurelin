use aurelin_framework::axum::{
    Json, Router,
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::{IntoResponse, Response},
    routing::get,
};
use leptos::prelude::LeptosOptions;

pub fn routes() -> Router<LeptosOptions> {
    Router::new().route("/up", get(health))
}

async fn health(headers: HeaderMap) -> Response {
    let expects_json = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|mime| mime.trim().starts_with("application/json"))
        });
    if expects_json {
        return (StatusCode::OK, Json(serde_json::json!({ "status": "up" }))).into_response();
    }
    (StatusCode::OK, "Application up").into_response()
}
