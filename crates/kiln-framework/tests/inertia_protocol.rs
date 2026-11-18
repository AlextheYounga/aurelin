use std::error::Error;

use kiln_framework::{
    axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode, header::CONTENT_TYPE},
        response::{Html, IntoResponse, Response},
        routing::get,
    },
    inertia::{
        Inertia, X_INERTIA,
        axum::{InertiaError, InertiaRequest, VersionLayer},
    },
};
use serde::Serialize;
use tower::ServiceExt;

#[derive(Serialize)]
struct WelcomeProps {
    greeting: &'static str,
}

async fn welcome(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(
        Inertia::response("Welcome", WelcomeProps { greeting: "Hello" }),
        |context| {
            Html(format!(
                "<!doctype html><html><body><script type=\"application/json\" data-page>{}</script><div id=\"app\"></div></body></html>",
                context.data_page()
            ))
            .into_response()
        },
    )
}

fn application() -> Router {
    Router::new().route("/", get(welcome)).layer(VersionLayer::new("assets-1"))
}

#[tokio::test]
async fn initial_visit_returns_html_with_script_safe_page_data() -> Result<(), Box<dyn Error>> {
    let response = application().oneshot(Request::builder().uri("/?name=%3Cscript%3E").body(Body::empty())?).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
    assert_eq!(response.headers()["vary"], "X-Inertia");
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("\"component\":\"Welcome\""));
    assert!(body.contains("\"version\":\"assets-1\""));
    assert!(body.contains("\"url\":\"/?name=%3Cscript%3E\""));
    assert!(!body.contains("<script>"));
    Ok(())
}

#[tokio::test]
async fn inertia_visit_returns_the_versioned_json_page() -> Result<(), Box<dyn Error>> {
    let response = application()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(X_INERTIA, "true")
                .header("X-Inertia-Version", "assets-1")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[X_INERTIA], "true");
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let page: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(page["component"], "Welcome");
    assert_eq!(page["props"]["greeting"], "Hello");
    assert_eq!(page["version"], "assets-1");
    Ok(())
}

#[tokio::test]
async fn stale_asset_version_forces_a_full_page_reload() -> Result<(), Box<dyn Error>> {
    let response = application()
        .oneshot(
            Request::builder()
                .uri("/dashboard?tab=profile")
                .header(X_INERTIA, "true")
                .header("X-Inertia-Version", "assets-old")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()["X-Inertia-Location"], "/dashboard?tab=profile");
    Ok(())
}
