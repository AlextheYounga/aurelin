#![allow(clippy::expect_used, clippy::too_many_arguments)]

use std::{
    error::Error,
    sync::atomic::{AtomicUsize, Ordering},
};

use aurelin_framework::{
    axum::{
        Form, Json, Router,
        body::{Body, to_bytes},
        extract::{Path, Query, Request},
        http::{StatusCode, header},
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    http::errors::AppError,
    validation::http::{Validated, ValidationResponse},
};
use serde::Deserialize;
use tower::ServiceExt;
use validator::{Validate, ValidationError};

static VALIDATIONS: AtomicUsize = AtomicUsize::new(0);
static EFFECTS: AtomicUsize = AtomicUsize::new(0);

fn check_name(value: &str) -> Result<(), ValidationError> {
    VALIDATIONS.fetch_add(1, Ordering::SeqCst);
    if value.len() < 3 { Err(ValidationError::new("short")) } else { Ok(()) }
}

#[derive(Deserialize, Validate)]
struct Input {
    #[validate(custom(function = "check_name"))]
    name: String,
}

async fn json(Validated(Json(_)): Validated<Json<Input>>) -> StatusCode {
    EFFECTS.fetch_add(1, Ordering::SeqCst);
    StatusCode::NO_CONTENT
}
async fn form(Validated(Form(_)): Validated<Form<Input>>) -> StatusCode {
    StatusCode::NO_CONTENT
}
async fn query(Validated(Query(_)): Validated<Query<Input>>) -> StatusCode {
    StatusCode::NO_CONTENT
}
async fn path(Validated(Path(_)): Validated<Path<Input>>) -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn custom(input: Result<Validated<Json<Input>>, ValidationResponse>) -> Response {
    match input {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error
            .with_safe_response(StatusCode::CONFLICT, "Please correct your input.")
            .with_safe_issue_messages(|_| Some("Please enter a longer name.".into()))
            .into_response(),
    }
}

async fn authorize(request: Request, next: Next) -> Response {
    if request.headers().contains_key("x-authorized") {
        next.run(request).await
    } else {
        AppError::Forbidden.into_response()
    }
}

fn app() -> Router {
    Router::new()
        .route("/json", post(json).route_layer(middleware::from_fn(authorize)))
        .route("/form", post(form))
        .route("/query", get(query))
        .route("/path/{name}", get(path))
        .route("/custom", post(custom))
        .route("/internal", get(|| async { AppError::Internal("private diagnostic".into()) }))
        .fallback(|| async { AppError::NotFound })
        .method_not_allowed_fallback(|| async { AppError::MethodNotAllowed })
}

async fn send(
    method: &str,
    uri: &str,
    content_type: Option<&str>,
    body: &str,
    authorized: bool,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if authorized {
        builder = builder.header("x-authorized", "yes");
    }
    let response = app().oneshot(builder.body(Body::from(body.to_owned())).expect("request")).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65536).await.expect("body");
    (status, String::from_utf8(bytes.to_vec()).expect("UTF-8"))
}

#[tokio::test]
async fn validation_and_errors_are_safe_at_public_http_boundaries() {
    let start_validation = VALIDATIONS.load(Ordering::SeqCst);
    let start_effects = EFFECTS.load(Ordering::SeqCst);
    assert_eq!(
        send("POST", "/json", Some("application/json"), r#"{"name":"x"}"#, false).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(VALIDATIONS.load(Ordering::SeqCst), start_validation);
    assert_eq!(EFFECTS.load(Ordering::SeqCst), start_effects);
    let (status, body) = send("POST", "/json", Some("application/json"), r#"{"name":"x"}"#, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let value: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(value["code"], "validation_failed");
    assert_eq!(value["issues"][0]["field"], "name");
    assert_eq!(value["issues"][0]["code"], "short");
    assert_eq!(EFFECTS.load(Ordering::SeqCst), start_effects);
    assert_eq!(
        send("POST", "/json", Some("application/json"), r#"{"name":"valid"}"#, true).await.0,
        StatusCode::NO_CONTENT
    );
    for (method, uri, content_type, input) in [
        ("POST", "/form", Some("application/x-www-form-urlencoded"), "name=x"),
        ("GET", "/query?name=x", None, ""),
        ("GET", "/path/x", None, ""),
    ] {
        assert_eq!(send(method, uri, content_type, input, true).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    }
    for (method, uri, content_type, input) in [
        ("POST", "/form", Some("application/x-www-form-urlencoded"), "name=valid"),
        ("GET", "/query?name=valid", None, ""),
        ("GET", "/path/valid", None, ""),
    ] {
        assert_eq!(send(method, uri, content_type, input, true).await.0, StatusCode::NO_CONTENT);
    }
    let (status, body) = send("POST", "/json", Some("application/json"), r#"{"name":"secret" invalid"#, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!body.contains("secret"));
    assert_eq!(send("POST", "/json", None, "{}", true).await.0, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        send("POST", "/json", Some("application/json"), r#"{"name":123}"#, true).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, body) = send("POST", "/custom", Some("application/json"), r#"{"name":"x"}"#, true).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let body: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(body["message"], "Please correct your input.");
    assert_eq!(body["issues"][0]["message"], "Please enter a longer name.");
    assert_eq!(send("GET", "/missing", None, "", true).await.0, StatusCode::NOT_FOUND);
    assert_eq!(send("POST", "/query", None, "", true).await.0, StatusCode::METHOD_NOT_ALLOWED);
    let (status, body) = send("GET", "/internal", None, "", true).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!body.contains("private diagnostic"));
    let error = AppError::Internal("private diagnostic".into());
    assert_eq!(error.source().map(ToString::to_string).as_deref(), Some("private diagnostic"));
}
