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
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

#[tokio::test]
async fn registration_login_throttling_and_logout_enforce_the_auth_boundaries()
-> Result<(), Box<dyn Error>> {
    let app = application(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
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
    assert_eq!(dashboard.status(), StatusCode::FOUND);
    assert_eq!(dashboard.headers()[header::LOCATION], "/email/verify");

    let verification = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/email/verify",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(verification.status(), StatusCode::OK);
    let authenticated_csrf = csrf_token(verification).await?;

    for uri in [
        "/login",
        "/register",
        "/forgot-password",
        "/reset-password/token?email=ada%40example.com",
    ] {
        let response = app
            .clone()
            .oneshot(request(
                Method::GET,
                uri,
                Some(&authenticated_cookie),
                None,
                None,
            )?)
            .await?;
        assert_eq!(response.status(), StatusCode::FOUND, "{uri}");
        assert_eq!(response.headers()[header::LOCATION], "/dashboard", "{uri}");
    }

    for (uri, body) in [
        (
            "/login",
            r#"{"email":"ada@example.com","password":"correct-horse"}"#,
        ),
        ("/register", registration_json()),
        ("/forgot-password", r#"{"email":"ada@example.com"}"#),
        (
            "/reset-password",
            r#"{"email":"ada@example.com","token":"invalid","password":"new-password","password_confirmation":"new-password"}"#,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                Method::POST,
                uri,
                Some(&authenticated_cookie),
                Some(&authenticated_csrf),
                Some(body),
            )?)
            .await?;
        assert_eq!(response.status(), StatusCode::SEE_OTHER, "{uri}");
        assert_eq!(response.headers()[header::LOCATION], "/dashboard", "{uri}");
    }

    for remaining in (0..6).rev() {
        let resent = app
            .clone()
            .oneshot(request(
                Method::POST,
                "/email/verification-notification",
                Some(&authenticated_cookie),
                Some(&authenticated_csrf),
                Some("{}"),
            )?)
            .await?;
        assert_eq!(resent.status(), StatusCode::SEE_OTHER);
        assert_eq!(resent.headers()[header::LOCATION], "/");
        assert_eq!(resent.headers()["x-ratelimit-limit"], "6");
        assert_eq!(
            resent.headers()["x-ratelimit-remaining"],
            remaining.to_string()
        );
    }
    let throttled_verification = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/email/verification-notification",
            Some(&authenticated_cookie),
            Some(&authenticated_csrf),
            Some("{}"),
        )?)
        .await?;
    assert_eq!(
        throttled_verification.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert!(
        throttled_verification
            .headers()
            .contains_key(header::RETRY_AFTER)
    );

    let confirmation = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/confirm-password",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(confirmation.status(), StatusCode::OK);

    let confirmation_status = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/confirmed-password-status",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert!(
        String::from_utf8(
            to_bytes(confirmation_status.into_body(), usize::MAX)
                .await?
                .to_vec(),
        )?
        .contains(r#""confirmed":false"#)
    );

    let confirmed = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/user/confirm-password",
            Some(&authenticated_cookie),
            Some(&authenticated_csrf),
            Some(r#"{"password":"correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(confirmed.status(), StatusCode::SEE_OTHER);

    let confirmation_status = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/confirmed-password-status",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert!(confirmation_status.headers().contains_key("x-retry-after"));
    assert!(
        String::from_utf8(
            to_bytes(confirmation_status.into_body(), usize::MAX)
                .await?
                .to_vec(),
        )?
        .contains(r#""confirmed":true"#)
    );

    let logged_out = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/logout",
            Some(&authenticated_cookie),
            Some(&authenticated_csrf),
            Some("{}"),
        )?)
        .await?;
    assert_eq!(logged_out.status(), StatusCode::SEE_OTHER);

    let unauthenticated_confirmation = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/confirm-password",
            None,
            None,
            None,
        )?)
        .await?;
    assert_eq!(unauthenticated_confirmation.status(), StatusCode::FOUND);
    assert_eq!(
        unauthenticated_confirmation.headers()[header::LOCATION],
        "/login"
    );

    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let login_cookie = session_cookie(&login_page)?;
    let login_csrf = csrf_token(login_page).await?;
    let guest_logout = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/logout",
            Some(&login_cookie),
            Some(&login_csrf),
            Some("{}"),
        )?)
        .await?;
    assert_eq!(guest_logout.status(), StatusCode::SEE_OTHER);
    assert_eq!(guest_logout.headers()[header::LOCATION], "/login");

    let failed_before_success = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(r#"{"email":"ada@example.com","password":"wrong-password"}"#),
        )?)
        .await?;
    assert_eq!(
        failed_before_success.headers()["x-ratelimit-remaining"],
        "4"
    );
    let successful_login = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(r#"{"email":"ada@example.com","password":"correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(successful_login.headers()[header::LOCATION], "/dashboard");
    let successful_login_cookie = session_cookie(&successful_login)?;
    let verification = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/email/verify",
            Some(&successful_login_cookie),
            None,
            None,
        )?)
        .await?;
    let successful_login_csrf = csrf_token(verification).await?;
    app.clone()
        .oneshot(request(
            Method::POST,
            "/logout",
            Some(&successful_login_cookie),
            Some(&successful_login_csrf),
            Some("{}"),
        )?)
        .await?;
    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let login_cookie = session_cookie(&login_page)?;
    let login_csrf = csrf_token(login_page).await?;

    for remaining in (0..5).rev() {
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
        assert_eq!(failed.headers()["x-ratelimit-limit"], "5");
        assert_eq!(
            failed.headers()["x-ratelimit-remaining"],
            remaining.to_string()
        );
    }
    let throttled = app
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(r#"{"email":"ada@example.com","password":"correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(throttled.headers()["x-ratelimit-limit"], "5");
    assert_eq!(throttled.headers()["x-ratelimit-remaining"], "0");
    assert!(throttled.headers().contains_key(header::RETRY_AFTER));
    assert!(throttled.headers().contains_key("x-ratelimit-reset"));
    Ok(())
}

#[tokio::test]
async fn failed_credentials_are_timeboxed_while_successful_credentials_return_early()
-> Result<(), Box<dyn Error>> {
    let minimum_duration = Duration::from_secs(2);
    let app = __AURELIN_CRATE__::bootstrap::application_with_options(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
        __AURELIN_CRATE__::bootstrap::ApplicationOptions {
            mailer: aurelin_framework::mail::Mailer::log(),
            authentication_timebox_duration: minimum_duration,
        },
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
            Some(registration_json()),
        )?)
        .await?;
    let authenticated_cookie = session_cookie(&registered)?;
    let verification_page = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/email/verify",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    let authenticated_csrf = csrf_token(verification_page).await?;
    app.clone()
        .oneshot(request(
            Method::POST,
            "/logout",
            Some(&authenticated_cookie),
            Some(&authenticated_csrf),
            Some("{}"),
        )?)
        .await?;
    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let cookie = session_cookie(&login_page)?;
    let csrf = csrf_token(login_page).await?;

    for body in [
        r#"{"email":"unknown@example.com","password":"wrong-password"}"#,
        r#"{"email":"ada@example.com","password":"wrong-password"}"#,
    ] {
        let started_at = Instant::now();
        let response = app
            .clone()
            .oneshot(request(
                Method::POST,
                "/login",
                Some(&cookie),
                Some(&csrf),
                Some(body),
            )?)
            .await?;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(started_at.elapsed() >= minimum_duration);
    }

    let started_at = Instant::now();
    let authenticated = app
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"email":"ada@example.com","password":"correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(authenticated.headers()[header::LOCATION], "/dashboard");
    assert!(started_at.elapsed() < minimum_duration);
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
