use std::{
    error::Error,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use aurelin_framework::axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use __AURELIN_CRATE__::{
    bootstrap::application,
    config::frontend::{DEV_SCRIPT, WELCOME_COMPONENT},
    frontend::Frontend,
};
use tower::ServiceExt;

#[tokio::test]
async fn startup_requires_explicit_migrations_for_memory_and_file_databases()
-> Result<(), Box<dyn Error>> {
    let error = application(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await
    .expect_err("an empty in-memory database must not be migrated at startup");
    assert!(error.to_string().contains("migrations are pending"));

    let directory = tempfile::tempdir()?;
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("test.sqlite").display()
    );
    let error = application(
        Frontend::new(),
        &url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await
    .expect_err("an empty database must not be migrated at startup");
    assert!(error.to_string().contains("migrations are pending"));
    let connection = aurelin_framework::database::connect(&url).await?;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let tables = connection.query_all(Statement::from_string(DbBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT IN ('seaql_migrations', 'sqlite_sequence')".to_owned())).await?;
    assert!(
        tables.is_empty(),
        "startup created application tables: {tables:?}"
    );
    __AURELIN_CRATE__::database::migrate(&url).await?;
    let _app = application(
        Frontend::new(),
        &url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn initial_visit_contains_the_inertia_page_and_vite_entry() -> Result<(), Box<dyn Error>> {
    let database_directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        database_directory.path().join("test.sqlite").display()
    );
    __AURELIN_CRATE__::database::migrate(&database_url).await?;
    let response = application(
        Frontend::new(),
        &database_url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?
    .oneshot(Request::builder().uri("/").body(Body::empty())?)
    .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
    assert!(body.contains(&format!(r#""component":"{WELCOME_COMPONENT}""#)));
    assert!(body.contains(&format!("/{DEV_SCRIPT}")));
    assert!(body.contains("data-page=\"app\""));
    Ok(())
}

#[tokio::test]
async fn inertia_visit_returns_json_for_the_same_component() -> Result<(), Box<dyn Error>> {
    let frontend = Frontend::new();
    let version = frontend.version().to_owned();
    let database_directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        database_directory.path().join("test.sqlite").display()
    );
    __AURELIN_CRATE__::database::migrate(&database_url).await?;
    let response = application(
        frontend,
        &database_url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?
    .oneshot(
        Request::builder()
            .uri("/")
            .header("X-Inertia", "true")
            .header("X-Inertia-Version", version)
            .body(Body::empty())?,
    )
    .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["component"], WELCOME_COMPONENT);
    assert_eq!(body["props"]["message"], "Welcome to Aurelin");
    Ok(())
}

#[tokio::test]
async fn vite_assets_are_proxied_at_the_urls_used_by_the_document() -> Result<(), Box<dyn Error>> {
    let vite = fake_vite_server()?;
    let database_directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        database_directory.path().join("test.sqlite").display()
    );
    __AURELIN_CRATE__::database::migrate(&database_url).await?;
    let response = application(
        Frontend::new(),
        &database_url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?
    .oneshot(
        Request::builder()
            .uri("/@vite/client")
            .body(Body::empty())?,
    )
    .await?;

    vite.join().map_err(|_| "fake Vite server panicked")??;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    assert_eq!(body.as_ref(), b"export default {};\n");
    Ok(())
}

fn fake_vite_server() -> Result<thread::JoinHandle<std::io::Result<()>>, Box<dyn Error>> {
    let listener = TcpListener::bind("[::]:5173")?;
    listener.set_nonblocking(true)?;
    Ok(thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        let mut request = [0; 1024];
        let bytes_read = stream.read(&mut request)?;
        assert!(
            String::from_utf8_lossy(&request[..bytes_read])
                .starts_with("GET /@vite/client HTTP/1.1")
        );
        let body = "export default {};\n";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: {}\r\nETag: test\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
        Ok(())
    }))
}
