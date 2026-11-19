use std::{
    env,
    error::Error,
    fs,
    io::ErrorKind,
    net::SocketAddr,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use aurelin_fortify::EmailVerification;
use aurelin_framework::axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, Response, StatusCode, header},
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tower::ServiceExt;

pub async fn register(app: &Router) -> Result<(String, String), Box<dyn Error>> {
    let page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let cookie = session_cookie(&page)?;
    let csrf = csrf_token(page).await?;
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
    Ok((session_cookie(&registered)?, csrf))
}

pub async fn verify_email(
    app: &Router,
    cookie: &str,
    application_key: &[u8],
    email: &str,
) -> Result<(), Box<dyn Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let url = EmailVerification::new(application_key)?.url(1, email, now)?;
    let response = app
        .clone()
        .oneshot(request(Method::GET, &url, Some(cookie), None, None)?)
        .await?;
    assert_eq!(response.status(), StatusCode::FOUND);
    Ok(())
}

pub async fn user_count(database: &sea_orm::DatabaseConnection) -> Result<i64, Box<dyn Error>> {
    let row = database
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM users",
        ))
        .await?
        .ok_or("user count query returned no row")?;
    Ok(row.try_get("", "count")?)
}

pub fn request(
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

pub fn assert_redirect(response: &Response<Body>, destination: &str) {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], destination);
}

pub fn response_cookie(response: &Response<Body>) -> Option<String> {
    response
        .headers()
        .get(header::SET_COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .next()
        .map(str::to_owned)
}

pub async fn response_body(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await?.to_vec(),
    )?)
}

pub fn temporary_database_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(env::temp_dir().join(format!(
        "aurelin-settings-{}-{}.sqlite",
        process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    )))
}

pub fn remove_sqlite_files(database_path: &Path) -> Result<(), Box<dyn Error>> {
    for suffix in ["", "-shm", "-wal"] {
        let path = format!("{}{suffix}", database_path.display());
        if let Err(error) = fs::remove_file(path)
            && error.kind() != ErrorKind::NotFound
        {
            return Err(error.into());
        }
    }
    Ok(())
}

fn session_cookie(response: &Response<Body>) -> Result<String, Box<dyn Error>> {
    response_cookie(response).ok_or_else(|| "missing session cookie".into())
}

async fn csrf_token(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    let body = response_body(response).await?;
    csrf_from_body(&body)
}

pub fn csrf_from_body(body: &str) -> Result<String, Box<dyn Error>> {
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}
