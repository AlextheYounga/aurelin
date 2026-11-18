use std::{error::Error, net::SocketAddr};

use kiln_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use __KILN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

#[tokio::test]
async fn registration_login_throttling_and_logout_enforce_the_auth_boundaries()
-> Result<(), Box<dyn Error>> {
    let app = application(Frontend::new(), "sqlite::memory:", false).await?;
    let register_page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let initial_cookie = session_cookie(&register_page)?;
    let csrf = csrf_token(register_page).await?;

    let rejected = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&initial_cookie),
            None,
            Some(registration_json()),
        )?)
        .await?;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

    let registered = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&initial_cookie),
            Some(&csrf),
            Some(registration_json()),
        )?)
        .await?;
    assert_eq!(registered.status(), StatusCode::SEE_OTHER);
    assert_eq!(registered.headers()[header::LOCATION], "/dashboard");
    let authenticated_cookie = session_cookie(&registered)?;
    assert_ne!(
        initial_cookie, authenticated_cookie,
        "login must rotate the session ID"
    );

    let dashboard = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/dashboard",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(dashboard.status(), StatusCode::OK);
    let logout_csrf = csrf_token(dashboard).await?;

    let logged_out = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/logout",
            Some(&authenticated_cookie),
            Some(&logout_csrf),
            Some("{}"),
        )?)
        .await?;
    assert_eq!(logged_out.status(), StatusCode::SEE_OTHER);

    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let login_cookie = session_cookie(&login_page)?;
    let login_csrf = csrf_token(login_page).await?;
    for _ in 0..6 {
        let failed = app
            .clone()
            .oneshot(request(
                Method::POST,
                "/login",
                Some(&login_cookie),
                Some(&login_csrf),
                Some(r#"{"email":"ada@example.com","password":"wrong-password"}"#),
            )?)
            .await?;
        assert_eq!(failed.status(), StatusCode::SEE_OTHER);
    }
    let throttled_page = app
        .oneshot(request(
            Method::GET,
            "/login",
            Some(&login_cookie),
            None,
            None,
        )?)
        .await?;
    let body = String::from_utf8(
        to_bytes(throttled_page.into_body(), usize::MAX)
            .await?
            .to_vec(),
    )?;
    assert!(body.contains("Too many login attempts"));
    Ok(())
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

fn registration_json() -> &'static str {
    r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#
}

fn session_cookie(response: &Response<Body>) -> Result<String, Box<dyn Error>> {
    let set_cookie = response.headers()[header::SET_COOKIE].to_str()?;
    Ok(set_cookie
        .split(';')
        .next()
        .ok_or("missing session cookie")?
        .to_owned())
}

async fn csrf_token(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}
