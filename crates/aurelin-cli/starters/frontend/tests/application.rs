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
async fn initial_visit_contains_the_inertia_page_and_vite_entry() -> Result<(), Box<dyn Error>> {
    let response = application(
        Frontend::new(),
        "sqlite::memory:",
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
    let response = application(
        frontend,
        "sqlite::memory:",
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
    let response = application(
        Frontend::new(),
        "sqlite::memory:",
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

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    assert_eq!(body.as_ref(), b"export default {};\n");
    vite.join().map_err(|_| "fake Vite server panicked")??;
    Ok(())
}

fn fake_vite_server() -> Result<thread::JoinHandle<std::io::Result<()>>, Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:5173")?;
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
