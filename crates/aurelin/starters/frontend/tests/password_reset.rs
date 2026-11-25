use std::{
    env,
    error::Error,
    fs,
    io::ErrorKind,
    net::SocketAddr,
    path::Path,
    process,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aurelin_framework::{
    auth::digest_token,
    axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Method, Request, Response, StatusCode, header},
    },
    events::EventDispatcher,
    mail::{Mailer, MessageSent},
};
use __AURELIN_CRATE__::{bootstrap::application_with_mailer, database, frontend::Frontend};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use tower::ServiceExt;

#[tokio::test]
async fn a_valid_reset_token_replaces_the_password_and_is_consumed() -> Result<(), Box<dyn Error>> {
    let database_path = env::temp_dir().join(format!(
        "aurelin-password-reset-{}-{}.sqlite",
        process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let database_url = format!("sqlite://{}?mode=rwc", database_path.display());
    database::migrate(&database_url).await?;
    let sent = Arc::new(AtomicUsize::new(0));
    let mut events = EventDispatcher::builder();
    let observed = Arc::clone(&sent);
    events.listen(move |_: Arc<MessageSent>| {
        let observed = Arc::clone(&observed);
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok::<(), std::convert::Infallible>(())
        }
    });
    let app = application_with_mailer(
        Frontend::new(),
        &database_url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
        Mailer::log().with_events(events.build()),
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
            Some(
                r#"{"name":"Ada Lovelace","email":"ada@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#,
            ),
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

    let forgot_page = app
        .clone()
        .oneshot(request(Method::GET, "/forgot-password", None, None, None)?)
        .await?;
    let reset_cookie = session_cookie(&forgot_page)?;
    let reset_csrf = csrf_token(forgot_page).await?;
    let sent_before_reset = sent.load(Ordering::SeqCst);
    let started = Instant::now();
    let unknown = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/forgot-password",
            Some(&reset_cookie),
            Some(&reset_csrf),
            Some(r#"{"email":"unknown@example.com"}"#),
        )?)
        .await?;
    // The timebox is a minimum; slow transports or loaded CI workers can take longer.
    assert!(started.elapsed() >= Duration::from_millis(190));
    assert_eq!(unknown.status(), StatusCode::SEE_OTHER);
    assert_eq!(sent.load(Ordering::SeqCst), sent_before_reset);
    let unknown_feedback = response_body(
        app.clone()
            .oneshot(request(
                Method::GET,
                "/forgot-password",
                Some(&reset_cookie),
                None,
                None,
            )?)
            .await?,
    )
    .await?;
    assert!(unknown_feedback.contains("We have emailed your password reset link."));
    let database = Database::connect(&database_url).await?;
    let unknown_records = database
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM password_reset_tokens WHERE email = ?",
            ["unknown@example.com".into()],
        ))
        .await?
        .ok_or("missing count")?
        .try_get::<i64>("", "count")?;
    assert_eq!(unknown_records, 0);
    database.close().await?;
    let requested = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/forgot-password",
            Some(&reset_cookie),
            Some(&reset_csrf),
            Some(r#"{"email":"ada@example.com"}"#),
        )?)
        .await?;
    assert_eq!(requested.status(), StatusCode::SEE_OTHER);
    assert_eq!(sent.load(Ordering::SeqCst), sent_before_reset + 1);
    assert_eq!(
        unknown.headers()[header::LOCATION],
        requested.headers()[header::LOCATION]
    );
    let known_feedback = response_body(
        app.clone()
            .oneshot(request(
                Method::GET,
                "/forgot-password",
                Some(&reset_cookie),
                None,
                None,
            )?)
            .await?,
    )
    .await?;
    assert_eq!(unknown_feedback, known_feedback);
    let database = Database::connect(&database_url).await?;
    let known_records = database
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM password_reset_tokens WHERE email = ?",
            ["ada@example.com".into()],
        ))
        .await?
        .ok_or("missing count")?
        .try_get::<i64>("", "count")?;
    assert_eq!(known_records, 1);
    database.close().await?;
    let throttled = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/forgot-password",
            Some(&reset_cookie),
            Some(&reset_csrf),
            Some(r#"{"email":"ada@example.com"}"#),
        )?)
        .await?;
    assert_eq!(throttled.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        unknown.headers()[header::LOCATION],
        throttled.headers()[header::LOCATION]
    );
    assert_eq!(
        response_body(unknown).await?,
        response_body(requested).await?
    );
    assert_eq!(
        unknown_feedback,
        response_body(
            app.clone()
                .oneshot(request(
                    Method::GET,
                    "/forgot-password",
                    Some(&reset_cookie),
                    None,
                    None,
                )?)
                .await?,
        )
        .await?
    );

    let token = "known-reset-token";
    let database = Database::connect(&database_url).await?;
    database
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE password_reset_tokens SET token_hash = ? WHERE email = ?",
            [digest_token(token).into(), "ada@example.com".into()],
        ))
        .await?;
    database.close().await?;

    let reset = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/reset-password",
            Some(&reset_cookie),
            Some(&reset_csrf),
            Some(
                r#"{"email":"ada@example.com","token":"known-reset-token","password":"new-correct-horse","password_confirmation":"new-correct-horse"}"#,
            ),
        )?)
        .await?;
    assert_eq!(reset.status(), StatusCode::SEE_OTHER);
    assert_eq!(reset.headers()[header::LOCATION], "/login");

    let reused = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/reset-password",
            Some(&reset_cookie),
            Some(&reset_csrf),
            Some(
                r#"{"email":"ada@example.com","token":"known-reset-token","password":"another-password","password_confirmation":"another-password"}"#,
            ),
        )?)
        .await?;
    assert_eq!(reused.status(), StatusCode::SEE_OTHER);
    assert_ne!(reused.headers()[header::LOCATION], "/login");

    let login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let login_cookie = session_cookie(&login_page)?;
    let login_csrf = csrf_token(login_page).await?;
    let logged_in = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&login_cookie),
            Some(&login_csrf),
            Some(r#"{"email":"ada@example.com","password":"new-correct-horse"}"#),
        )?)
        .await?;
    assert_eq!(logged_in.status(), StatusCode::SEE_OTHER);
    assert_eq!(logged_in.headers()[header::LOCATION], "/dashboard");

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
    let body = response_body(response).await?;
    let marker = r#""csrf_token":""#;
    let start = body.find(marker).ok_or("missing CSRF token")? + marker.len();
    let end = body[start..].find('"').ok_or("unterminated CSRF token")? + start;
    Ok(body[start..end].to_owned())
}

async fn response_body(response: Response<Body>) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await?.to_vec(),
    )?)
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
