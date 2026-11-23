use std::{error::Error, net::SocketAddr};

use aurelin_framework::auth::CompromisedPasswordChecker;
use aurelin_framework::axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{HeaderMap, Method, Request, Response, StatusCode, header},
    routing::get,
    serve,
};
use __AURELIN_CRATE__::{
    bootstrap::application,
    config::password_policy::{PasswordPolicy, PasswordPolicyViolation},
    frontend::Frontend,
};
use tokio::net::TcpListener;
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
        "http://localhost:8000",
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
    assert_password_policy_error(register_page).await?;
    Ok(())
}

#[tokio::test]
async fn production_rejects_compromised_passwords_and_accepts_clean_passwords()
-> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = Router::new().route("/{prefix}", get(hibp_range));
    let _server = tokio::spawn(async move { serve(listener, server).await });
    let policy = PasswordPolicy::with_compromised_password_checker(
        "production",
        CompromisedPasswordChecker::for_endpoint(format!("http://{address}")),
    );

    assert_eq!(
        policy.validate("Compromised-1!").await,
        Err(PasswordPolicyViolation::Compromised)
    );
    assert_eq!(policy.validate("Uncompromised-1!").await, Ok(()));
    Ok(())
}

async fn hibp_range(headers: HeaderMap) -> (StatusCode, &'static str) {
    if headers
        .get("add-padding")
        .and_then(|value| value.to_str().ok())
        != Some("true")
    {
        return (StatusCode::BAD_REQUEST, "");
    }
    (
        StatusCode::OK,
        "40FCDB5DF35B1118285478DED924654A4B8:4\r\n00000000000000000000000000000000000:0",
    )
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

async fn assert_password_policy_error(response: Response<Body>) -> Result<(), Box<dyn Error>> {
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains("uppercase and one lowercase"));
    Ok(())
}

fn csrf_token_from_body(body: &str) -> Result<String, Box<dyn Error>> {
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}
