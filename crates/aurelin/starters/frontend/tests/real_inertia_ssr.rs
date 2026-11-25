use std::{
    collections::HashMap,
    error::Error,
    net::TcpStream,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use aurelin_framework::{
    axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    },
    view::inertia_ssr::{Ssr, SsrSettings},
};

use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use tower::ServiceExt;

struct Renderer(Child);
impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "requires npm ci, npm run build:ssr, Node 22+ and free localhost port 13714"]
async fn official_renderer_serves_html_and_shuts_down() -> Result<(), Box<dyn Error>> {
    if TcpStream::connect("127.0.0.1:13714").is_ok() {
        return Err("SSR test requires port 13714 to be free".into());
    }
    let mut renderer = Renderer(
        Command::new("node")
            .arg("bootstrap/ssr/ssr.js")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let mut ready = false;
    for _ in 0..100 {
        if TcpStream::connect("127.0.0.1:13714").is_ok() {
            ready = true;
            break;
        }
        if renderer.0.try_wait()?.is_some() {
            return Err("SSR renderer exited during startup".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    if !ready {
        return Err("SSR renderer did not start".into());
    }
    let health = Command::new("npm").args(["run", "health:ssr"]).output()?;
    assert!(health.status.success());
    assert!(String::from_utf8(health.stdout)?.contains("OK"));
    let mut env = HashMap::new();
    env.insert("INERTIA_SSR_ENABLED".into(), "true".into());
    env.insert("INERTIA_SSR_STRICT".into(), "true".into());
    let frontend = Frontend::new().with_ssr(Ssr::new(SsrSettings::from_environment(&env)?)?);
    let version = frontend.version();
    let database_directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        database_directory.path().join("test.sqlite").display()
    );
    __AURELIN_CRATE__::database::migrate(&database_url).await?;
    let app = application(
        frontend,
        &database_url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let visit = |url| Request::builder().uri(url).body(Body::empty());
    let (a, b) = tokio::join!(
        app.clone().oneshot(visit("/?case=one")?),
        app.clone().oneshot(visit("/?case=two")?)
    );
    for (response, expected, unexpected) in
        [(a?, "case=one", "case=two"), (b?, "case=two", "case=one")]
    {
        assert_eq!(response.status(), StatusCode::OK);
        let html = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
        assert!(html.contains("<h1>Welcome to Aurelin</h1>"));
        assert!(html.contains("Welcome - Aurelin</title>"));
        assert!(html.contains("data-server-rendered=\"true\""));
        assert!(html.contains(expected));
        assert!(!html.contains(unexpected));
    }
    let login = app.clone().oneshot(visit("/login")?).await?;
    assert_eq!(login.status(), StatusCode::OK);
    let login_html = String::from_utf8(to_bytes(login.into_body(), usize::MAX).await?.to_vec())?;
    assert!(login_html.contains("Log in to your account"));
    assert!(login_html.contains("data-server-rendered=\"true\""));
    let shutdown = Command::new("npm").args(["run", "stop:ssr"]).status()?;
    assert!(shutdown.success());
    assert!(renderer.0.wait()?.success());
    let json = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("X-Inertia", "true")
                .header("X-Inertia-Version", version)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(json.status(), StatusCode::OK);
    assert!(json.headers().contains_key("X-Inertia"));
    let strict = app.oneshot(visit("/")?).await?;
    assert_eq!(strict.status(), StatusCode::INTERNAL_SERVER_ERROR);
    env.insert("INERTIA_SSR_STRICT".into(), "false".into());
    let frontend = Frontend::new().with_ssr(Ssr::new(SsrSettings::from_environment(&env)?)?);
    let app = application(
        frontend,
        &database_url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let fallback = app.oneshot(visit("/")?).await?;
    assert_eq!(fallback.status(), StatusCode::OK);
    let html = String::from_utf8(to_bytes(fallback.into_body(), usize::MAX).await?.to_vec())?;
    assert!(html.contains("data-page=\"app\""));
    assert!(!html.contains("data-server-rendered=\"true\""));
    Ok(())
}
