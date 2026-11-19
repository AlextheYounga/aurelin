use std::error::Error;

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use __AURELIN_CRATE__::{
    bootstrap::application,
    frontend::Frontend,
    frontend_config::{DEV_SCRIPT, WELCOME_COMPONENT},
};
use tower::ServiceExt;

#[tokio::test]
async fn initial_visit_contains_the_inertia_page_and_vite_entry() -> Result<(), Box<dyn Error>> {
    let response = application(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
    )
    .await?
    .oneshot(Request::builder().uri("/").body(Body::empty())?)
    .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains(&format!(r#""component":"{WELCOME_COMPONENT}""#)));
    assert!(body.contains(&format!("/{DEV_SCRIPT}")));
    assert!(body.contains("data-page=\"app\""));
    Ok(())
}

#[tokio::test]
async fn inertia_visit_returns_json_for_the_same_component() -> Result<(), Box<dyn Error>> {
    let frontend = Frontend::new();
    let version = frontend.version().to_owned();
    let response = application(frontend, "sqlite::memory:", false, b"test-key", "local")
        .await?
        .oneshot(
            Request::builder()
                .uri("/")
                .header("X-Inertia", "true")
                .header("X-Inertia-Version", version)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["component"], WELCOME_COMPONENT);
    assert_eq!(body["props"]["message"], "Welcome to Aurelin");
    Ok(())
}
