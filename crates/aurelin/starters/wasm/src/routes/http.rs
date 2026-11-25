use aurelin_framework::{axum::Router, events::EventDispatcher, health};
use leptos::prelude::LeptosOptions;

pub fn routes() -> Router<LeptosOptions> {
    Router::new().route("/up", health::route(EventDispatcher::builder().build()))
}
