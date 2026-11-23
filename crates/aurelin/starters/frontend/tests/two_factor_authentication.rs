use std::{
    error::Error,
    net::SocketAddr,
    time::{Duration, Instant},
};

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use __AURELIN_CRATE__::{
    bootstrap::{ApplicationOptions, application_with_options},
    frontend::Frontend,
};
use totp_rs::{Builder, Secret};
use tower::ServiceExt;

#[tokio::test]
async fn confirmed_two_factor_setup_requires_a_code_or_single_use_recovery_code_at_login()
-> Result<(), Box<dyn Error>> {
    let authentication_timebox_duration = Duration::from_secs(2);
    let app = application_with_options(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
        ApplicationOptions {
            mailer: aurelin_framework::mail::Mailer::log(),
            authentication_timebox_duration,
        },
    )
    .await?;
    let register_page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let initial_cookie = session_cookie(&register_page)?;
    let csrf = csrf_token(register_page).await?;
    let registered = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&initial_cookie),
            Some(&csrf),
            Some(r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#),
        )?)
        .await?;
    let authenticated_cookie = session_cookie(&registered)?;
    let confirmation_page = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/confirm-password",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    let csrf = csrf_token(confirmation_page).await?;
    let confirmed = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/user/confirm-password",
            Some(&authenticated_cookie),
            Some(&csrf),
            Some(r#"{"password":"correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(confirmed.status(), StatusCode::SEE_OTHER);

    let enabled = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/user/two-factor-authentication",
            Some(&authenticated_cookie),
            Some(&csrf),
            Some("{}"),
        )?)
        .await?;
    assert_eq!(enabled.status(), StatusCode::SEE_OTHER);
    assert_eq!(enabled.headers()[header::LOCATION], "/settings/security");
    let secret = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/two-factor-secret-key",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    let secret: serde_json::Value =
        serde_json::from_slice(&to_bytes(secret.into_body(), usize::MAX).await?)?;
    let secret =
        Secret::try_from_base32(secret["secretKey"].as_str().ok_or("missing secret key")?)?;
    let code = Builder::new()
        .with_secret(secret)
        .build()?
        .generate_current()
        .to_string();
    let two_factor_confirmed = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/user/confirmed-two-factor-authentication",
            Some(&authenticated_cookie),
            Some(&csrf),
            Some(&format!(r#"{{"code":"{code}"}}"#)),
        )?)
        .await?;
    assert_eq!(two_factor_confirmed.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        two_factor_confirmed.headers()[header::LOCATION],
        "/settings/security"
    );
    let recovery_codes = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/two-factor-recovery-codes",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    let recovery_codes: Vec<String> =
        serde_json::from_slice(&to_bytes(recovery_codes.into_body(), usize::MAX).await?)?;
    assert_eq!(recovery_codes.len(), 8);

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
    let login_cookie = session_cookie(&login_page)?;
    let login_csrf = csrf_token(login_page).await?;
    let login_started_at = Instant::now();
    let password_accepted = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(r#"{"email":"ada@example.com","password":"correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(password_accepted.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        password_accepted.headers()[header::LOCATION],
        "/two-factor-challenge"
    );
    assert!(login_started_at.elapsed() < authentication_timebox_duration);

    for remaining in (1..5).rev() {
        let invalid = app
            .clone()
            .oneshot(request(
                Method::POST,
                "/two-factor-challenge",
                Some(&login_cookie),
                Some(&login_csrf),
                Some(r#"{"code":"000000"}"#),
            )?)
            .await?;
        assert_eq!(invalid.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            invalid.headers()["x-ratelimit-remaining"],
            remaining.to_string()
        );
    }
    let authenticated = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/two-factor-challenge",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(&format!(r#"{{"recovery_code":"{}"}}"#, recovery_codes[0])),
        )?)
        .await?;
    assert_eq!(authenticated.status(), StatusCode::SEE_OTHER);
    assert_eq!(authenticated.headers()[header::LOCATION], "/dashboard");
    let rotated_cookie = session_cookie(&authenticated)?;
    assert_ne!(login_cookie, rotated_cookie);
    let verification_page = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/email/verify",
            Some(&rotated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(verification_page.status(), StatusCode::OK);
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
