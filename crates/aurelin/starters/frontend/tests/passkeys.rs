use std::{error::Error, net::SocketAddr};

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

#[tokio::test]
async fn passkey_options_follow_laravels_guest_and_password_confirmed_contract()
-> Result<(), Box<dyn Error>> {
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
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let guest_cookie = session_cookie(&login_page)?;
    let csrf = csrf_token(login_page).await?;
    let endpoints = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/.well-known/passkey-endpoints",
            None,
            None,
            None,
        )?)
        .await?;
    assert_eq!(endpoints.status(), StatusCode::OK);
    let endpoints: serde_json::Value =
        serde_json::from_slice(&to_bytes(endpoints.into_body(), usize::MAX).await?)?;
    assert_eq!(
        endpoints["enroll"],
        "http://localhost:8000/settings/security"
    );
    assert_eq!(endpoints["manage"], endpoints["enroll"]);
    let login_options = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/passkeys/login/options",
            Some(&guest_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(login_options.status(), StatusCode::OK);
    let login_options: serde_json::Value =
        serde_json::from_slice(&to_bytes(login_options.into_body(), usize::MAX).await?)?;
    assert!(login_options["options"].get("challenge").is_some());
    assert_eq!(login_options["options"]["rpId"], "localhost");
    assert_eq!(login_options["options"]["userVerification"], "required");
    assert!(login_options["options"].get("publicKey").is_none());

    let registered = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&guest_cookie),
            Some(&csrf),
            Some(r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#),
        )?)
        .await?;
    let authenticated_cookie = session_cookie(&registered)?;
    let blocked = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/passkeys/options",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    let blocked_registration = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/user/passkeys",
            Some(&authenticated_cookie),
            Some(&csrf),
            Some(r#"{"name":"Laptop","credential":{}}"#),
        )?)
        .await?;
    assert_eq!(blocked_registration.status(), StatusCode::FORBIDDEN);

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
    let registration_options = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/user/passkeys/options",
            Some(&authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(registration_options.status(), StatusCode::OK);
    let registration_options: serde_json::Value =
        serde_json::from_slice(&to_bytes(registration_options.into_body(), usize::MAX).await?)?;
    assert_eq!(
        registration_options["options"]["user"]["name"],
        "ada@example.com"
    );
    assert_eq!(
        registration_options["options"]["authenticatorSelection"]["residentKey"],
        "required"
    );
    assert_eq!(
        registration_options["options"]["authenticatorSelection"]["userVerification"],
        "required"
    );
    Ok(())
}

#[tokio::test]
async fn passkey_routes_are_limited_to_ten_attempts_per_identifier_and_address()
-> Result<(), Box<dyn Error>> {
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
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let cookie = session_cookie(&login_page)?;

    for remaining in (0..10).rev() {
        let response = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/passkeys/login/options",
                Some(&cookie),
                None,
                None,
            )?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-ratelimit-limit"], "10");
        assert_eq!(
            response.headers()["x-ratelimit-remaining"],
            remaining.to_string()
        );
    }
    let throttled = app
        .oneshot(request(
            Method::GET,
            "/passkeys/login/options",
            Some(&cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(throttled.headers().contains_key(header::RETRY_AFTER));
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
