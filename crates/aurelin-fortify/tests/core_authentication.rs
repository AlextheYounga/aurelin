//! A core-only consumer implements no passkey or two-factor contracts.
#![cfg(not(any(feature = "passkeys", feature = "two-factor-authentication")))]

mod support;

use std::{error::Error, net::SocketAddr};

use aurelin_fortify::axum::routes;
use aurelin_framework::{
    axum::{
        Extension,
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Request, StatusCode, header},
    },
    session::web_session_layer,
};
use axum_login::AuthManagerLayerBuilder;
use tower::ServiceExt;
use tower_sessions::MemoryStore;

use support::{Backend, Frontend};

#[tokio::test]
async fn core_only_consumer_logs_in_reads_authenticated_dashboard_and_logs_out() -> Result<(), Box<dyn Error>> {
    let sessions = web_session_layer(MemoryStore::default(), "session", false);
    let app = routes::<Frontend, Backend>()
        .with_state(Frontend)
        .layer(AuthManagerLayerBuilder::new(Backend, sessions).build())
        .layer(Extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345)))));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"email":"ada@example.com","password":"test-password"}"#))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/dashboard");
    let cookie = response.headers()[header::SET_COOKIE].to_str()?.split(';').next().ok_or("missing cookie")?.to_owned();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .header(header::COOKIE, &cookie)
                .header("X-Inertia", "true")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let page: serde_json::Value = serde_json::from_slice(&to_bytes(response.into_body(), 65536).await?)?;
    assert_eq!(page["props"]["auth"]["user"], "ada@example.com");
    let response = app
        .clone()
        .oneshot(Request::builder().method("POST").uri("/logout").header(header::COOKIE, &cookie).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/dashboard").header(header::COOKIE, &cookie).body(Body::empty())?)
        .await?;
    assert_eq!(response.headers()[header::LOCATION], "/login");
    for route in ["/passkeys/login/options", "/two-factor-challenge"] {
        let response = app.clone().oneshot(Request::builder().uri(route).body(Body::empty())?).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    Ok(())
}
