use aurelin_framework::axum::Router;

/// Compose routes, application state, and middleware here.
pub fn application() -> Router {
    crate::routes::http::routes()
}
