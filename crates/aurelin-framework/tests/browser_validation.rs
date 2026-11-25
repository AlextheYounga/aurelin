use std::error::Error;

use aurelin_framework::{
    axum::{
        Form, Json, Router,
        body::{Body, to_bytes},
        extract::Request,
        http::{StatusCode, header},
        middleware::{self, Next},
        response::{Html, IntoResponse, Response},
        routing::{get, post},
    },
    inertia::{
        Inertia,
        axum::{InertiaError, InertiaRequest},
    },
    session::web_session_layer,
    validation::browser::{BrowserValidated, OldInput, SafeOldInput, flash_middleware, validation_page_props},
};
use serde::Deserialize;
use tower::ServiceExt;
use tower_sessions::MemoryStore;
use validator::Validate;

#[derive(Deserialize, Validate)]
struct Input {
    #[validate(length(min = 4))]
    name: String,
    password: String,
    token: String,
}

impl OldInput for Input {
    fn safe_old_input(&self) -> SafeOldInput {
        SafeOldInput::default().field("name", &self.name)
    }
}

async fn form(BrowserValidated(Form(input)): BrowserValidated<Form<Input>>) -> StatusCode {
    let _ = (&input.password, &input.token);
    StatusCode::NO_CONTENT
}

async fn json(BrowserValidated(Json(input)): BrowserValidated<Json<Input>>) -> StatusCode {
    let _ = (&input.password, &input.token);
    StatusCode::NO_CONTENT
}

#[derive(Deserialize, Validate)]
struct Address {
    #[validate(email)]
    email: String,
}

#[derive(Deserialize, Validate)]
struct Nested {
    #[validate(nested)]
    addresses: Vec<Address>,
}

impl OldInput for Nested {
    fn safe_old_input(&self) -> SafeOldInput {
        SafeOldInput::default()
    }
}

async fn nested(BrowserValidated(Json(_)): BrowserValidated<Json<Nested>>) -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn page(request: InertiaRequest) -> Result<Response, InertiaError> {
    let props = validation_page_props(&request, Default::default());
    request.render(Inertia::response("Form", props), |context| Html(context.data_page().to_owned()).into_response())
}

async fn deny(_request: Request, _next: Next) -> StatusCode {
    StatusCode::FORBIDDEN
}

fn app() -> Router {
    Router::new()
        .route("/form", get(page).post(form).patch(form))
        .route("/json", post(json))
        .route("/nested", post(nested))
        .route("/forbidden", post(form).route_layer(middleware::from_fn(deny)))
        .layer(middleware::from_fn(flash_middleware))
        .layer(web_session_layer(MemoryStore::default(), "test_session", false))
}

#[tokio::test]
async fn inertia_nested_errors_use_indexed_field_names_after_redirect() -> Result<(), Box<dyn Error>> {
    let app = app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/nested")
                .header(header::CONTENT_TYPE, "application/json")
                .header("X-Inertia", "true")
                .header(header::REFERER, "/form")
                .body(Body::from(r#"{"addresses":[{"email":"bad"},{"email":"bad2"}]}"#))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::FOUND);
    let cookie = response.headers()[header::SET_COOKIE].to_str()?.split(';').next().ok_or("cookie")?;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/form")
                .header(header::COOKIE, cookie)
                .header("X-Inertia", "true")
                .body(Body::empty())?,
        )
        .await?;
    let page: serde_json::Value = serde_json::from_str(&body(response).await?)?;
    assert_eq!(
        page["props"]["errors"]["addresses.0.email"], "The addresses.0.email field must be a valid email address.",
        "{page}"
    );
    assert_eq!(
        page["props"]["errors"]["addresses.1.email"],
        "The addresses.1.email field must be a valid email address."
    );
    Ok(())
}

async fn body(response: Response) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(to_bytes(response.into_body(), 65536).await?.to_vec())?)
}

#[tokio::test]
async fn browser_and_inertia_validation_redirect_and_flash_only_explicit_safe_fields() -> Result<(), Box<dyn Error>> {
    for bag in [None, Some("profile")] {
        let app = app();
        let mut request = Request::builder()
            .method("PATCH")
            .uri("/form")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::REFERER, "http://example.test/form?tab=1")
            .header(header::HOST, "example.test")
            .header("X-Inertia", "true")
            .header(header::ACCEPT, "application/json");
        if let Some(bag) = bag {
            request = request.header("X-Inertia-Error-Bag", bag);
        }
        let response =
            app.clone().oneshot(request.body(Body::from("name=xy&password=hiddenpass&token=hiddentoken"))?).await?;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], "/form?tab=1");
        let cookie = response.headers()[header::SET_COOKIE].to_str()?.split(';').next().ok_or("cookie")?.to_owned();
        assert!(!body(response).await?.contains("hidden"));
        let get = || {
            Request::builder()
                .uri("/form")
                .header(header::COOKIE, &cookie)
                .header("X-Inertia", "true")
                .body(Body::empty())
        };
        let page = body(app.clone().oneshot(get()?).await?).await?;
        assert!(!page.contains("hiddenpass"));
        assert!(!page.contains("hiddentoken"));
        let page: serde_json::Value = serde_json::from_str(&page)?;
        let errors = &page["props"]["errors"];
        let errors = match bag {
            Some(bag) => &errors[bag],
            None => errors,
        };
        assert_eq!(errors["name"], "The name field has an invalid length.");
        assert_eq!(page["props"]["old"]["name"], "xy");
        assert!(page["props"]["old"].get("password").is_none());
        let next: serde_json::Value = serde_json::from_str(&body(app.oneshot(get()?).await?).await?)?;
        assert_eq!(next["props"]["errors"], serde_json::json!({}));
        assert_eq!(next["props"]["old"], serde_json::json!({}));
    }
    Ok(())
}

#[tokio::test]
async fn api_errors_remain_json_and_authorization_precedes_validation() -> Result<(), Box<dyn Error>> {
    let app = app();
    let payload = r#"{"name":"xy","password":"hiddenpass","token":"hiddentoken"}"#;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/json")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let text = body(response).await?;
    assert!(text.contains("validation_failed"));
    assert!(!text.contains("hidden"));
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/json")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"password":"hidden" invalid"#))?,
        )
        .await?;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert!(!body(malformed).await?.contains("hidden"));
    let wrong_type =
        app.clone().oneshot(Request::builder().method("POST").uri("/json").body(Body::from("{}"))?).await?;
    assert_eq!(wrong_type.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/forbidden")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("name=x&password=hidden&token=hidden"))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    Ok(())
}

#[tokio::test]
async fn ordinary_browser_posts_redirect_and_reject_external_referers() -> Result<(), Box<dyn Error>> {
    let app = app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/form")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::HOST, "example.test")
                .header(header::REFERER, "https://attacker.test/steal")
                .body(Body::from("name=x&password=hidden&token=hidden"))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers()[header::LOCATION], "/");
    let cookie = response.headers()[header::SET_COOKIE].to_str()?.split(';').next().ok_or("cookie")?.to_owned();
    assert!(!body(response).await?.contains("hidden"));
    let page = app.oneshot(Request::builder().uri("/form").header(header::COOKIE, cookie).body(Body::empty())?).await?;
    assert_eq!(page.headers()[header::CONTENT_TYPE], "text/html; charset=utf-8");
    let html = body(page).await?;
    assert!(html.contains("The name field has an invalid length."));
    assert!(!html.contains("hidden"));
    Ok(())
}
