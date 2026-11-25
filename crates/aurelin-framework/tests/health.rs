use std::{
    convert::Infallible,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use aurelin_framework::{
    axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    },
    events::EventDispatcher,
    health::{self, DiagnosingHealth},
};
use tower::ServiceExt;

#[tokio::test]
async fn health_route_invokes_registered_diagnostic_and_returns_up() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut events = EventDispatcher::builder();
    let captured = Arc::clone(&calls);
    events.listen(move |_: Arc<DiagnosingHealth>| {
        let captured = Arc::clone(&captured);
        async move {
            captured.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>(())
        }
    });
    let app = Router::new().route("/up", health::route(events.build()));
    let response = app
        .oneshot(Request::get("/up").header("accept", "application/json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(&to_bytes(response.into_body(), usize::MAX).await.unwrap()[..], br#"{"status":"up"}"#);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn health_route_hides_diagnostic_failure_and_returns_down() {
    let mut events = EventDispatcher::builder();
    events.listen(|_: Arc<DiagnosingHealth>| async { Err::<(), _>(io::Error::other("private diagnostic detail")) });
    let app = Router::new().route("/up", health::route(events.build()));
    let response = app
        .oneshot(Request::get("/up").header("accept", "application/json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(&to_bytes(response.into_body(), usize::MAX).await.unwrap()[..], br#"{"status":"down"}"#);
}
