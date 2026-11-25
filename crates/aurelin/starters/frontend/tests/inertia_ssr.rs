use std::{
    collections::HashMap,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use aurelin_framework::{
    axum::{
        Json, Router,
        body::{Body, to_bytes},
        extract::State,
        http::{Request, StatusCode},
        routing::post,
    },
    view::inertia_ssr::{Ssr, SsrSettings},
};

use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[tokio::test]
async fn application_and_fortify_html_use_ssr_but_inertia_visits_do_not()
-> Result<(), Box<dyn Error>> {
    let calls = Arc::new(AtomicUsize::new(0));
    let renderer = Router::new().route("/render", post(|State(calls): State<Arc<AtomicUsize>>, Json(page): Json<Value>| async move {
        calls.fetch_add(1, Ordering::SeqCst);
        Json(json!({"head": ["<title>SSR title</title>"], "body": format!("<main>{}</main>", page["component"])}))
    })).with_state(calls.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move { aurelin_framework::axum::serve(listener, renderer).await });
    let mut environment = HashMap::new();
    environment.insert("INERTIA_SSR_ENABLED".into(), "true".into());
    environment.insert("INERTIA_SSR_URL".into(), format!("http://{address}/render"));
    let frontend =
        Frontend::new().with_ssr(Ssr::new(SsrSettings::from_environment(&environment)?)?);
    let version = frontend.version();
    let app = application(
        frontend,
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    for path in ["/", "/login"] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let html = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?;
        assert!(html.contains("<title>SSR title</title>"));
        assert!(html.contains("<main>"));
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/login")
                .header("X-Inertia", "true")
                .header("X-Inertia-Version", version)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    Ok(())
}
