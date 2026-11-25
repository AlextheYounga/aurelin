#[allow(dead_code)]
mod support;

use __AURELIN_CRATE__::{bootstrap::application, database, frontend::Frontend};
use aurelin_framework::axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use std::error::Error;
use support::settings::*;
use tower::ServiceExt;

#[tokio::test]
async fn token_migration_can_be_applied_rolled_back_and_reapplied() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    let url = format!("sqlite://{}?mode=rwc", path.display());
    database::migrate(&url).await?;
    let db = Database::connect(&url).await?;
    let has_table = || async {
        db.query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'personal_access_tokens'",
        ))
        .await
        .map(|row| row.is_some())
    };
    assert!(has_table().await?);
    database::rollback(&url).await?;
    assert!(!has_table().await?);
    database::migrate(&url).await?;
    assert!(has_table().await?);
    drop(db);
    remove_sqlite_files(&path)?;
    Ok(())
}

#[tokio::test]
async fn issued_token_authenticates_bearer_only_and_can_be_revoked() -> Result<(), Box<dyn Error>> {
    let path = temporary_database_path()?;
    let url = format!("sqlite://{}?mode=rwc", path.display());
    database::migrate(&url).await?;
    let app = application(
        Frontend::new(),
        &url,
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let (cookie, csrf) = register(&app).await?;
    let issue = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/tokens",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"name":"test","abilities":["read"]}"#),
        )?)
        .await?;
    assert_eq!(issue.status(), StatusCode::CREATED);
    let response: serde_json::Value =
        serde_json::from_slice(&to_bytes(issue.into_body(), usize::MAX).await?)?;
    let token = response["token"].as_str().ok_or("missing token")?;
    let id = response["id"].as_u64().ok_or("missing token id")?;
    let second_page = app
        .clone()
        .oneshot(request(Method::GET, "/register", None, None, None)?)
        .await?;
    let second_cookie = response_cookie(&second_page).ok_or("missing second session")?;
    let second_csrf = csrf_from_body(&response_body(second_page).await?)?;
    let second_registration = app.clone().oneshot(request(Method::POST, "/register", Some(&second_cookie), Some(&second_csrf), Some(r#"{"name":"Grace Hopper","email":"grace@example.com","password":"correct-horse","password_confirmation":"correct-horse"}"#))?).await?;
    let second_cookie =
        response_cookie(&second_registration).ok_or("missing authenticated second session")?;
    let cross_owner = app
        .clone()
        .oneshot(request(
            Method::DELETE,
            &format!("/tokens/{id}"),
            Some(&second_cookie),
            Some(&second_csrf),
            None,
        )?)
        .await?;
    assert_eq!(cross_owner.status(), StatusCode::FORBIDDEN);
    let db = Database::connect(&url).await?;
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT digest FROM personal_access_tokens",
        ))
        .await?
        .ok_or("missing digest")?;
    let digest: String = row.try_get("", "digest")?;
    assert_ne!(digest, token);
    assert!(!token.contains(&digest));
    let authenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/user")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(authenticated.status(), StatusCode::OK);
    let user = String::from_utf8(
        to_bytes(authenticated.into_body(), usize::MAX)
            .await?
            .to_vec(),
    )?;
    assert!(!user.contains(token) && !user.contains(&digest) && !user.contains("password"));
    let listing = app
        .clone()
        .oneshot(request(Method::GET, "/tokens", Some(&cookie), None, None)?)
        .await?;
    assert_eq!(listing.status(), StatusCode::OK);
    let listing = response_body(listing).await?;
    assert!(!listing.contains(token) && !listing.contains(&digest));
    let wildcard = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/tokens",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"name":"wildcard","abilities":["*"]}"#),
        )?)
        .await?;
    assert_eq!(wildcard.status(), StatusCode::FORBIDDEN);
    let expired = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/tokens",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"name":"expired","abilities":[],"expires_at":1}"#),
        )?)
        .await?;
    assert_eq!(expired.status(), StatusCode::CREATED);
    let expired: serde_json::Value =
        serde_json::from_slice(&to_bytes(expired.into_body(), usize::MAX).await?)?;
    let expired = expired["token"].as_str().ok_or("missing expired token")?;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/user")
                .header(header::AUTHORIZATION, format!("Bearer {expired}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/user")
                .header(header::AUTHORIZATION, "Bearer malformed")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(malformed.status(), StatusCode::UNAUTHORIZED);
    let cookie_only = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/api/user",
            Some(&cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(cookie_only.status(), StatusCode::UNAUTHORIZED);
    let query = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/user?access_token=x")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(query.status(), StatusCode::UNAUTHORIZED);
    let no_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/tokens")
                .header(header::COOKIE, &cookie)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"x","abilities":[]}"#))?,
        )
        .await?;
    assert_ne!(no_csrf.status(), StatusCode::CREATED);
    let revoke_request = Request::builder()
        .method(Method::DELETE)
        .uri("/api/token")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())?;
    let concurrent_request = Request::builder()
        .uri("/api/user")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())?;
    let (revoke, concurrent) = tokio::join!(
        app.clone().oneshot(revoke_request),
        app.clone().oneshot(concurrent_request)
    );
    let revoke = revoke?;
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);
    assert!(matches!(
        concurrent?.status(),
        StatusCode::OK | StatusCode::UNAUTHORIZED
    ));
    let after = app
        .oneshot(
            Request::builder()
                .uri("/api/user")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);
    drop(db);
    remove_sqlite_files(&path)?;
    Ok(())
}
