use std::{
    env,
    error::Error,
    fs,
    io::ErrorKind,
    net::SocketAddr,
    path::Path,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use aurelin_framework::{
    auth::BcryptPasswordHasher,
    axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Method, Request, Response, header},
    },
};
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use tower::ServiceExt;

#[tokio::test]
async fn successful_login_rehashes_a_password_with_outdated_bcrypt_parameters()
-> Result<(), Box<dyn Error>> {
    let database_path = env::temp_dir().join(format!(
        "aurelin-password-rehash-{}-{}.sqlite",
        process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let database_url = format!("sqlite://{}?mode=rwc", database_path.display());
    let app = application(
        Frontend::new(),
        &database_url,
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
    let registration_cookie = session_cookie(&register_page)?;
    let registration_csrf = csrf_token(register_page).await?;
    let registered = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/register",
            Some(&registration_cookie),
            Some(&registration_csrf),
            Some(r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#),
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

    let database = Database::connect(&database_url).await?;
    let old_hash = BcryptPasswordHasher::new(4).hash("correct-horse").await?;
    database
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE users SET password_hash = ? WHERE email = ?",
            [old_hash.into(), "ada@example.com".into()],
        ))
        .await?;
    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let login_cookie = session_cookie(&login_page)?;
    let login_csrf = csrf_token(login_page).await?;
    app.clone()
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(r#"{"email":"ada@example.com","password":"correct-horse"}"#),
        )?)
        .await?;

    let row = database
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT password_hash FROM users WHERE email = 'ada@example.com'",
        ))
        .await?
        .ok_or("registered user is missing")?;
    let upgraded_hash = row.try_get::<String>("", "password_hash")?;
    assert!(upgraded_hash.starts_with("$2b$12$"));

    drop(database);
    drop(app);
    remove_sqlite_files(&database_path)?;
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

fn remove_sqlite_files(database_path: &Path) -> Result<(), Box<dyn Error>> {
    for path in [
        database_path.to_path_buf(),
        database_path.with_extension("sqlite-shm"),
        database_path.with_extension("sqlite-wal"),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
