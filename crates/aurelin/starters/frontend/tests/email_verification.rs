use std::{
    error::Error,
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};

use aurelin_fortify::EmailVerification;
use aurelin_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

#[tokio::test]
async fn a_signed_link_verifies_the_authenticated_users_email() -> Result<(), Box<dyn Error>> {
    let application_key = b"test-key";
    let database_directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        database_directory.path().join("test.sqlite").display()
    );
    __AURELIN_CRATE__::database::migrate(&database_url).await?;
    let app = application(
        Frontend::new(),
        &database_url,
        false,
        application_key,
        "local",
        "http://localhost:8000",
    )
    .await?;
    let register_page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let cookie = session_cookie(&register_page)?;
    let csrf = csrf_token(register_page).await?;
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
    let authenticated_cookie = session_cookie(&registered)?;
    let mut resend_request = request(
        Method::POST,
        "/email/verification-notification",
        Some(&authenticated_cookie),
        Some(&csrf),
        Some("{}"),
    )?;
    resend_request.headers_mut().insert(
        header::REFERER,
        "http://localhost:8000/email/verify".parse()?,
    );
    let resent = app.clone().oneshot(resend_request).await?;
    assert_eq!(resent.status(), StatusCode::SEE_OTHER);
    assert_eq!(resent.headers()[header::LOCATION], "/email/verify");

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let invalid_hash_url =
        EmailVerification::new(application_key)?.url(1, "wrong@example.com", now)?;
    let invalid_hash = app
        .clone()
        .oneshot(request(
            Method::GET,
            &invalid_hash_url,
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(invalid_hash.status(), StatusCode::FORBIDDEN);

    let invalid_user_url =
        EmailVerification::new(application_key)?.url(123, "ada@example.com", now)?;
    let invalid_user = app
        .clone()
        .oneshot(request(
            Method::GET,
            &invalid_user_url,
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(invalid_user.status(), StatusCode::FORBIDDEN);

    let verification_url =
        EmailVerification::new(application_key)?.url(1, "ada@example.com", now)?;
    let verified = app
        .clone()
        .oneshot(request(
            Method::GET,
            &verification_url,
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(verified.status(), StatusCode::FOUND);
    assert_eq!(
        verified.headers()[header::LOCATION],
        "/dashboard?verified=1"
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

    let verification_prompt = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/email/verify",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(verification_prompt.status(), StatusCode::FOUND);
    assert_eq!(
        verification_prompt.headers()[header::LOCATION],
        "/dashboard"
    );

    let already_verified = app
        .oneshot(request(
            Method::GET,
            &verification_url,
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(already_verified.status(), StatusCode::FOUND);
    assert_eq!(
        already_verified.headers()[header::LOCATION],
        "/dashboard?verified=1"
    );
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
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}
