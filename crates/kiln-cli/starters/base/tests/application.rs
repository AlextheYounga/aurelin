use std::error::Error;

use kiln_framework::axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use __KILN_CRATE__::bootstrap::application;
use tower::ServiceExt;

#[tokio::test]
async fn home_route_returns_the_welcome_response() -> Result<(), Box<dyn Error>> {
    let response = application()
        .oneshot(Request::builder().uri("/").body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await?,
        "Welcome to Kiln"
    );
    Ok(())
}

#[tokio::test]
async fn unknown_route_returns_not_found() -> Result<(), Box<dyn Error>> {
    let response = application()
        .oneshot(Request::builder().uri("/missing").body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn health_route_reports_up_as_json_when_requested() -> Result<(), Box<dyn Error>> {
    let response = application()
        .oneshot(
            Request::builder()
                .uri("/up")
                .header("accept", "application/json")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await?,
        r#"{"status":"up"}"#
    );
    Ok(())
}
