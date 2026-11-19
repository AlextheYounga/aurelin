mod support;

use std::error::Error;

use aurelin_framework::axum::http::{Method, StatusCode, header};
use __AURELIN_CRATE__::{bootstrap::application, frontend::Frontend};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use support::settings::*;
use tower::ServiceExt;

#[tokio::test]
async fn settings_update_profile_password_and_delete_the_account() -> Result<(), Box<dyn Error>> {
    let database_path = temporary_database_path()?;
    let database_url = format!("sqlite://{}?mode=rwc", database_path.display());
    let application_key = b"test-key";
    let app = application(
        Frontend::new(),
        &database_url,
        false,
        application_key,
        "local",
        "http://localhost:8000",
    )
    .await?;
    let (cookie, csrf) = register(&app).await?;
    verify_email(&app, &cookie, application_key, "ada@example.com").await?;

    assert_eq!(
        app.clone()
            .oneshot(request(
                Method::GET,
                "/settings/profile",
                Some(&cookie),
                None,
                None,
            )?)
            .await?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                Method::GET,
                "/settings/appearance",
                Some(&cookie),
                None,
                None,
            )?)
            .await?
            .status(),
        StatusCode::OK
    );

    let updated = app
        .clone()
        .oneshot(request(
            Method::PATCH,
            "/settings/profile",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"name":"Augusta Ada King","email":"ADA.NEW@EXAMPLE.COM"}"#),
        )?)
        .await?;
    assert_redirect(&updated, "/settings/profile");
    let database = Database::connect(&database_url).await?;
    let user = database
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name, email, email_verified_at FROM users WHERE id = 1",
        ))
        .await?
        .ok_or("updated user is missing")?;
    assert_eq!(user.try_get::<String>("", "name")?, "Augusta Ada King");
    assert_eq!(user.try_get::<String>("", "email")?, "ada.new@example.com");
    assert_eq!(user.try_get::<Option<i64>>("", "email_verified_at")?, None);

    let unverified_appearance = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/settings/appearance",
            Some(&cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(unverified_appearance.status(), StatusCode::FOUND);
    assert_eq!(
        unverified_appearance.headers()[header::LOCATION],
        "/email/verify"
    );
    verify_email(&app, &cookie, application_key, "ada.new@example.com").await?;

    let renamed = app
        .clone()
        .oneshot(request(
            Method::PATCH,
            "/settings/profile",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"name":"Ada King","email":"ADA.NEW@EXAMPLE.COM"}"#),
        )?)
        .await?;
    assert_redirect(&renamed, "/settings/profile");
    let user = database
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT email_verified_at FROM users WHERE id = 1",
        ))
        .await?
        .ok_or("renamed user is missing")?;
    assert!(
        user.try_get::<Option<i64>>("", "email_verified_at")?
            .is_some()
    );

    database
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO users (name, email, password_hash, created_at, updated_at) \
             SELECT 'Taken User', 'taken@example.com', password_hash, created_at, updated_at \
             FROM users WHERE id = 1",
        ))
        .await?;
    let duplicate = app
        .clone()
        .oneshot(request(
            Method::PATCH,
            "/settings/profile",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"name":"Ada","email":"TAKEN@EXAMPLE.COM"}"#),
        )?)
        .await?;
    assert_redirect(&duplicate, "/settings/profile");
    let profile_with_error = response_body(
        app.clone()
            .oneshot(request(
                Method::GET,
                "/settings/profile",
                Some(&cookie),
                None,
                None,
            )?)
            .await?,
    )
    .await?;
    assert!(profile_with_error.contains("The email has already been taken."));

    let guarded_security = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/settings/security",
            Some(&cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(guarded_security.status(), StatusCode::FOUND);
    assert_eq!(
        guarded_security.headers()[header::LOCATION],
        "/user/confirm-password"
    );
    let confirmed = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/user/confirm-password",
            Some(&cookie),
            Some(&csrf),
            Some(r#"{"password":"correct-horse"}"#),
        )?)
        .await?;
    assert_redirect(&confirmed, "/settings/security");
    let security_page = response_body(
        app.clone()
            .oneshot(request(
                Method::GET,
                "/settings/security",
                Some(&cookie),
                None,
                None,
            )?)
            .await?,
    )
    .await?;
    assert!(
        security_page.contains("settings/security") || security_page.contains("settings/Security")
    );
    assert!(security_page.contains(r#""canManagePasskeys":true"#));
    assert!(security_page.contains(r#""canManageTwoFactor":true"#));
    assert!(security_page.contains(r#""twoFactorEnabled":false"#));

    let second_login_page = app
        .clone()
        .oneshot(request(Method::GET, "/login", None, None, None)?)
        .await?;
    let second_login_cookie = response_cookie(&second_login_page).ok_or("missing login cookie")?;
    let second_login_csrf = csrf_from_body(&response_body(second_login_page).await?)?;
    let second_login = app
        .clone()
        .oneshot(request(
            Method::POST,
            "/login",
            Some(&second_login_cookie),
            Some(&second_login_csrf),
            Some(r#"{"email":"ada.new@example.com","password":"correct-horse"}"#),
        )?)
        .await?;
    let second_authenticated_cookie =
        response_cookie(&second_login).ok_or("missing second authenticated cookie")?;

    let wrong_password = app
        .clone()
        .oneshot(request(
            Method::PUT,
            "/settings/password",
            Some(&cookie),
            Some(&csrf),
            Some(
                r#"{"current_password":"wrong-password","password":"new-correct-horse","password_confirmation":"new-correct-horse"}"#,
            ),
        )?)
        .await?;
    assert_redirect(&wrong_password, "/settings/security");

    let password_updated = app
        .clone()
        .oneshot(request(
            Method::PUT,
            "/settings/password",
            Some(&cookie),
            Some(&csrf),
            Some(
                r#"{"current_password":"correct-horse","password":"new-correct-horse","password_confirmation":"new-correct-horse"}"#,
            ),
        )?)
        .await?;
    assert_redirect(&password_updated, "/settings/security");
    assert_eq!(password_updated.headers()["x-ratelimit-limit"], "6");
    let current_cookie = response_cookie(&password_updated).unwrap_or(cookie);
    let invalidated_other_session = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/settings/profile",
            Some(&second_authenticated_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(invalidated_other_session.status(), StatusCode::FOUND);
    assert_eq!(
        invalidated_other_session.headers()[header::LOCATION],
        "/login"
    );

    let wrong_deletion = app
        .clone()
        .oneshot(request(
            Method::DELETE,
            "/settings/profile",
            Some(&current_cookie),
            Some(&csrf),
            Some(r#"{"password":"wrong-password"}"#),
        )?)
        .await?;
    assert_redirect(&wrong_deletion, "/settings/profile");
    assert_eq!(user_count(&database).await?, 2);

    let deleted = app
        .clone()
        .oneshot(request(
            Method::DELETE,
            "/settings/profile",
            Some(&current_cookie),
            Some(&csrf),
            Some(r#"{"password":"new-correct-horse"}"#),
        )?)
        .await?;
    assert_redirect(&deleted, "/");
    assert_eq!(user_count(&database).await?, 1);
    let former_session = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/settings/profile",
            Some(&current_cookie),
            None,
            None,
        )?)
        .await?;
    assert_eq!(former_session.status(), StatusCode::FOUND);
    assert_eq!(former_session.headers()[header::LOCATION], "/login");

    database.close().await?;
    drop(app);
    remove_sqlite_files(&database_path)?;
    Ok(())
}

#[tokio::test]
async fn password_updates_are_limited_to_six_attempts_per_minute() -> Result<(), Box<dyn Error>> {
    let app = application(
        Frontend::new(),
        "sqlite::memory:",
        false,
        b"test-key",
        "local",
        "http://localhost:8000",
    )
    .await?;
    let (cookie, csrf) = register(&app).await?;
    verify_email(&app, &cookie, b"test-key", "ada@example.com").await?;

    for remaining in (0..6).rev() {
        let response = app
            .clone()
            .oneshot(request(
                Method::PUT,
                "/settings/password",
                Some(&cookie),
                Some(&csrf),
                Some(
                    r#"{"current_password":"wrong","password":"new-correct-horse","password_confirmation":"new-correct-horse"}"#,
                ),
            )?)
            .await?;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers()["x-ratelimit-remaining"],
            remaining.to_string()
        );
    }
    let throttled = app
        .oneshot(request(
            Method::PUT,
            "/settings/password",
            Some(&cookie),
            Some(&csrf),
            Some(
                r#"{"current_password":"wrong","password":"new-correct-horse","password_confirmation":"new-correct-horse"}"#,
            ),
        )?)
        .await?;
    assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(throttled.headers().contains_key(header::RETRY_AFTER));
    Ok(())
}
