use std::{error::Error, net::SocketAddr};

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

#[tokio::test]
async fn remember_me_extends_the_authenticated_session_cookie() -> Result<(), Box<dyn Error>> {
    let app = application(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let (authenticated_cookie, csrf) = register(&app).await?;
    let logged_out = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/logout",
            Some(&authenticated_cookie),
            Some(&csrf),
            Some("{}"),
        )?)
        .await?;
    assert_eq!(logged_out.status(), StatusCode::SEE_OTHER);

    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let login_cookie = response_cookie(&login_page).ok_or("missing login session cookie")?;
    let login_body = response_body(login_page).await?;
    let marker = r#""csrf_token":""#;
    let start = login_body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = login_body[start..]
        .find('"')
        .ok_or("unterminated CSRF token")?
        + start;
    let login_csrf = &login_body[start..end];

    let remembered = app
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(login_csrf),
            Some(r#"{"email":"ada@example.com","password":"correct-horse","remember":"on"}"#),
        )?)
        .await?;
    assert_eq!(remembered.status(), StatusCode::SEE_OTHER);
    assert_eq!(remembered.headers()[header::LOCATION], "/dashboard");
    assert!(
        remembered.headers()[header::SET_COOKIE]
            .to_str()?
            .contains("Max-Age=157680000")
    );
    Ok(())
}

async fn register(
    app: &aurelin_framework::axum::Router,
) -> Result<(String, String), Box<dyn Error>> {
    let page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let cookie = response_cookie(&page).ok_or("missing registration cookie")?;
    let body = response_body(page).await?;
    let csrf = csrf_token(&body)?;
    let registered = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&cookie),
            Some(&csrf),
            Some(
                r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#,
            ),
        )?)
        .await?;
    let authenticated_cookie =
        response_cookie(&registered).ok_or("missing authenticated cookie")?;
    Ok((authenticated_cookie, csrf))
}

fn request(
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<&str>,
) -> Result<Request<Body>, Box<dyn Error>> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .extension(ConnectInfo("127.0.0.1:45000".parse::<SocketAddr>()?));
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    Ok(request.body(Body::from(body.unwrap_or_default().to_owned()))?)
}

fn response_cookie(response: &Response<Body>) -> Option<String> {
    response
        .headers()
        .get(header::SET_COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .next()
        .map(str::to_owned)
}

async fn response_body(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await?.to_vec(),
    )?)
}

fn csrf_token(body: &str) -> Result<String, Box<dyn Error>> {
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}
