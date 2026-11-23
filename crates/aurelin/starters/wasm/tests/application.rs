use std::error::Error;

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{
        Request, StatusCode,
        header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
    },
};
use leptos::server_fn::ServerFn;
use tower::ServiceExt;

async fn application() -> Result<aurelin_framework::axum::Router, Box<dyn Error>> {
    let options = leptos::config::get_configuration(Some("Cargo.toml"))?.leptos_options;
    __AURELIN_CRATE__::bootstrap::application(
        options,
        "sqlite::memory:",
        false,
        b"test-application-key-with-at-least-thirty-two-bytes",
        "testing",
        "http://localhost:8000",
        aurelin_framework::mail::Mailer::log(),
    )
    .await
}

#[tokio::test]
async fn login_route_returns_server_rendered_html() -> Result<(), Box<dyn Error>> {
    let response = application()
        .await?
        .oneshot(Request::builder().uri("/login").body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(
        body.contains("<h1 id=\"login-heading\">Log in</h1>"),
        "{body}"
    );
    assert!(
        body.contains("<script type=\"module\"") && body.contains("/pkg/"),
        "{body}"
    );
    Ok(())
}

#[tokio::test]
async fn health_route_reports_up_as_json_when_requested() -> Result<(), Box<dyn Error>> {
    let response = application()
        .await?
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

#[tokio::test]
async fn login_server_function_rejects_an_invalid_csrf_token() -> Result<(), Box<dyn Error>> {
    let mut request = Request::builder()
        .method("POST")
        .uri(<__AURELIN_CRATE__::app::Login as ServerFn>::PATH)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "email=user%40example.com&password=secret&csrf_token=invalid",
        ))?;
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:3000".parse::<std::net::SocketAddr>()?,
    ));

    let response = application().await?.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}

#[tokio::test]
async fn dashboard_server_function_rejects_a_guest() -> Result<(), Box<dyn Error>> {
    let response = application()
        .await?
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(<__AURELIN_CRATE__::app::DashboardUser as ServerFn>::PATH)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::empty())?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/login")
    );
    Ok(())
}

#[tokio::test]
async fn registration_creates_an_authenticated_session_that_can_log_out()
-> Result<(), Box<dyn Error>> {
    let application = application().await?;
    let registration_page = application
        .clone()
        .oneshot(Request::builder().uri("/register").body(Body::empty())?)
        .await?;
    let cookie = response_cookie(&registration_page)?;
    let body = String::from_utf8(
        to_bytes(registration_page.into_body(), usize::MAX)
            .await?
            .to_vec(),
    )?;
    let csrf_token = hidden_input_value(&body, "csrf_token")?;

    let registration = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(<__AURELIN_CRATE__::app::Register as ServerFn>::PATH)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(COOKIE, &cookie)
                .body(Body::from(format!(
                    "name=Aurelin+User&email=user%40example.com&password=correct-horse-battery-staple&password_confirmation=correct-horse-battery-staple&csrf_token={csrf_token}"
                )))?,
        )
        .await?;
    assert_eq!(
        registration
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/dashboard")
    );
    let authenticated_cookie = response_cookie(&registration).unwrap_or(cookie);

    let dashboard = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(<__AURELIN_CRATE__::app::DashboardUser as ServerFn>::PATH)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(COOKIE, &authenticated_cookie)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        dashboard
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/email/verify")
    );

    let logout = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(<__AURELIN_CRATE__::app::Logout as ServerFn>::PATH)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(COOKIE, &authenticated_cookie)
                .body(Body::from(format!("csrf_token={csrf_token}")))?,
        )
        .await?;
    assert_eq!(
        logout
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );
    let logged_out_cookie = response_cookie(&logout).unwrap_or(authenticated_cookie);
    let dashboard = application
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(<__AURELIN_CRATE__::app::DashboardUser as ServerFn>::PATH)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(COOKIE, logged_out_cookie)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        dashboard
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/login")
    );
    Ok(())
}

fn response_cookie(
    response: &aurelin_framework::axum::response::Response,
) -> Result<String, Box<dyn Error>> {
    Ok(response
        .headers()
        .get(SET_COOKIE)
        .ok_or("response did not set a session cookie")?
        .to_str()?
        .split(';')
        .next()
        .ok_or("session cookie was empty")?
        .to_owned())
}

fn hidden_input_value(body: &str, name: &str) -> Result<String, Box<dyn Error>> {
    let marker = format!("name=\"{name}\" value=\"");
    let value = body
        .split_once(&marker)
        .and_then(|(_, remainder)| remainder.split_once('\"'))
        .map(|(value, _)| value)
        .ok_or("hidden input was not rendered")?;
    Ok(value.to_owned())
}
