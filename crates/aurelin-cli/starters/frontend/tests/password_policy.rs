use std::{error::Error, net::SocketAddr};

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

#[tokio::test]
async fn production_registration_requires_laravels_local_password_rules()
-> Result<(), Box<dyn Error>> {
    let app = application(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "production",
    )
    .await?;
    let register_page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let cookie = session_cookie(&register_page)?;
    let csrf = csrf_token(register_page).await?;

    let rejected = app
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
    assert_eq!(rejected.status(), StatusCode::SEE_OTHER);
    assert_eq!(rejected.headers()[header::LOCATION], "/register");

    let register_page = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/register",
            Some(&cookie),
            None,
            None,
        )?)
        .await?;
    let csrf = csrf_token_and_assert_error(register_page).await?;
    let registered = app
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&cookie),
            Some(&csrf),
            Some(
                r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"Correct-Horse-1","password_confirmation":"Correct-Horse-1"}"#,
            ),
        )?)
        .await?;
    assert_eq!(registered.status(), StatusCode::SEE_OTHER);
    assert_eq!(registered.headers()[header::LOCATION], "/dashboard");
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

fn session_cookie(response: &Response<Body>) -> Result<String, Box<dyn Error>> {
    let set_cookie = response.headers()[header::SET_COOKIE].to_str()?;
    Ok(set_cookie
        .split(';')
        .next()
        .ok_or("missing session cookie")?
        .to_owned())
}

async fn csrf_token(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    csrf_token_from_body(
        String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?.as_str(),
    )
}

async fn csrf_token_and_assert_error(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("uppercase and one lowercase"));
    csrf_token_from_body(&body)
}

fn csrf_token_from_body(body: &str) -> Result<String, Box<dyn Error>> {
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}
