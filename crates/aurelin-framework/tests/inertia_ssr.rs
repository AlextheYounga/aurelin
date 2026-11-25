use std::{error::Error, time::Duration};

use aurelin_framework::{
    axum::{Json, Router, http::StatusCode, routing::post},
    inertia::HtmlResponseContext,
    view::inertia_ssr::{SsrClient, SsrConfig, SsrError},
};
use serde_json::{Value, json};
use tokio::{net::TcpListener, time::sleep};

async fn renderer(router: Router, timeout: Duration) -> Result<SsrClient, Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move { axum::serve(listener, router).await });
    Ok(SsrClient::new(SsrConfig { url: format!("http://{address}/render").parse()?, timeout })?)
}

#[tokio::test]
async fn renderer_receives_the_exact_page_and_isolates_concurrent_visits() {
    let client = renderer(
        Router::new().route(
            "/render",
            post(
                |Json(page): Json<Value>| async move { Json(json!({"head": [page["url"]], "body": page.to_string()})) },
            ),
        ),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    let first = json!({"component": "Welcome", "props": {"name": "</script>"}, "url": "/one", "version": "v1"});
    let second = json!({"component": "Welcome", "props": {"name": "other"}, "url": "/two", "version": "v1"});
    let first_context = HtmlResponseContext::new(first.to_string().replace('<', "\\u003c"));
    let second_context = HtmlResponseContext::new(second.to_string());
    let (a, b) = tokio::join!(client.render(&first_context), client.render(&second_context));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.head, ["/one"]);
    assert_eq!(b.head, ["/two"]);
    assert_eq!(serde_json::from_str::<Value>(&a.body).unwrap(), first);
    assert_eq!(serde_json::from_str::<Value>(&b.body).unwrap(), second);
}

#[tokio::test]
async fn unsuccessful_or_malformed_renderer_responses_are_errors() {
    let context = HtmlResponseContext::new("{}");
    let failure = renderer(
        Router::new().route("/render", post(|| async { StatusCode::INTERNAL_SERVER_ERROR })),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert!(matches!(failure.render(&context).await, Err(SsrError::Status(StatusCode::INTERNAL_SERVER_ERROR))));
    let malformed = renderer(
        Router::new().route("/render", post(|| async { Json(json!({"head": "invalid", "body": ""})) })),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert!(matches!(malformed.render(&context).await, Err(SsrError::Response(_))));
}

#[tokio::test]
async fn unresponsive_renderer_times_out() {
    let client = renderer(
        Router::new().route(
            "/render",
            post(|| async {
                sleep(Duration::from_secs(1)).await;
                Json(json!({"head": [], "body": ""}))
            }),
        ),
        Duration::from_millis(30),
    )
    .await
    .unwrap();
    assert!(
        matches!(client.render(&HtmlResponseContext::new("{}")).await, Err(SsrError::Request(error)) if error.is_timeout())
    );
}
