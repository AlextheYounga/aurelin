use aurelin_framework::{
    axum::{Router, routing::get},
    events::EventDispatcher,
    health,
};

pub fn routes() -> Router {
    Router::new()
        .route("/", get(welcome))
        .route("/up", health::route(EventDispatcher::builder().build()))
}

async fn welcome() -> &'static str {
    "Welcome to Aurelin"
}
