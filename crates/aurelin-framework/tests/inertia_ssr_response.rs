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
        middleware,
        response::Response,
        routing::{get, post},
    },
    inertia::{
        HtmlResponseContext, Inertia,
        axum::{InertiaError, InertiaRequest, VersionLayer},
    },
    view::inertia_ssr::{Ssr, SsrResponse, SsrSettings, document_response, render_document},
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower::ServiceExt;

fn document(context: &HtmlResponseContext, rendered: Option<&SsrResponse>) -> String {
    match rendered {
        Some(rendered) => format!("<head>{}</head>{}", rendered.head.join(""), rendered.body),
        None => format!("<head></head><script>{}</script><div id=app></div>", context.data_page()),
    }
}

async fn page(request: InertiaRequest) -> Result<Response, InertiaError> {
    request.render(Inertia::response("Welcome", json!({"message": "</script>"})), |context| {
        document_response(context, document)
    })
}

async fn app(strict: bool) -> Result<(Router, Arc<AtomicUsize>), Box<dyn Error>> {
    let calls = Arc::new(AtomicUsize::new(0));
    let server = Router::new()
        .route(
            "/render",
            post(|State(calls): State<Arc<AtomicUsize>>, Json(page): Json<Value>| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                if page["url"] == "/fail" {
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({})));
                }
                (
                    StatusCode::OK,
                    Json(json!({"head": ["<title>Rendered</title>"], "body": format!("<main>{}</main>", page["url"])})),
                )
            }),
        )
        .with_state(calls.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move { axum::serve(listener, server).await });
    let mut env = HashMap::new();
    env.insert("INERTIA_SSR_ENABLED".into(), "true".into());
    env.insert("INERTIA_SSR_STRICT".into(), strict.to_string());
    env.insert("INERTIA_SSR_URL".into(), format!("http://{address}/render"));
    let ssr = Ssr::new(SsrSettings::from_environment(&env)?)?;
    let router = Router::new()
        .route("/{*path}", get(page))
        .layer(VersionLayer::new("v1"))
        .layer(middleware::from_fn_with_state(ssr, render_document));
    Ok((router, calls))
}

async fn body(response: Response) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(to_bytes(response.into_body(), usize::MAX).await?.to_vec())?)
}

#[tokio::test]
async fn html_is_rendered_and_json_and_stale_visits_skip_the_renderer() {
    let (app, calls) = app(false).await.unwrap();
    let response = app.clone().oneshot(Request::builder().uri("/welcome").body(Body::empty()).unwrap()).await.unwrap();
    let html = body(response).await.unwrap();
    assert!(html.contains("<title>Rendered</title>"));
    assert!(html.contains("<main>\"/welcome\"</main>"));
    for version in ["v1", "stale"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/welcome")
                    .header("X-Inertia", "true")
                    .header("X-Inertia-Version", version)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), if version == "v1" { StatusCode::OK } else { StatusCode::CONFLICT });
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failure_falls_back_safely_or_returns_strict_error() {
    for strict in [false, true] {
        let (app, _) = app(strict).await.unwrap();
        let response = app.oneshot(Request::builder().uri("/fail").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), if strict { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::OK });
        if !strict {
            assert!(body(response).await.unwrap().contains("\\u003C/script\\u003E"));
        }
    }
}

#[tokio::test]
async fn concurrent_html_visits_keep_their_own_page() {
    let (app, _) = app(false).await.unwrap();
    let request = |path| Request::builder().uri(path).body(Body::empty()).unwrap();
    let (one, two) = tokio::join!(app.clone().oneshot(request("/one")), app.oneshot(request("/two")));
    assert!(body(one.unwrap()).await.unwrap().contains("/one"));
    assert!(body(two.unwrap()).await.unwrap().contains("/two"));
}
