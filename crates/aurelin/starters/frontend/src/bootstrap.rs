use aurelin_framework::{
    axum::{Router, middleware},
    inertia::axum::VersionLayer,
};
use axum_login::AuthManagerLayerBuilder;
use axum_tower_sessions_csrf::CsrfMiddleware;
use tower_sessions_sqlx_store::SqliteStore;

use aurelin_framework::mail::Mailer;
use std::time::Duration;

use crate::{auth::AuthBackend, frontend::Frontend};

pub struct ApplicationOptions {
    pub mailer: Mailer,
    pub authentication_timebox_duration: Duration,
}

impl Default for ApplicationOptions {
    fn default() -> Self {
        Self {
            mailer: Mailer::log(),
            authentication_timebox_duration:
                aurelin_fortify::axum::DEFAULT_AUTHENTICATION_TIMEBOX_DURATION,
        }
    }
}

/// Compose routes, application state, and middleware here.
pub async fn application(
    frontend: Frontend,
    database_url: &str,
    secure_cookie: bool,
    application_key: &[u8],
    application_environment: &str,
    application_url: &str,
) -> Result<Router, Box<dyn std::error::Error>> {
    application_with_options(
        frontend,
        database_url,
        secure_cookie,
        application_key,
        application_environment,
        application_url,
        ApplicationOptions::default(),
    )
    .await
}

pub async fn application_with_mailer(
    frontend: Frontend,
    database_url: &str,
    secure_cookie: bool,
    application_key: &[u8],
    application_environment: &str,
    application_url: &str,
    mailer: Mailer,
) -> Result<Router, Box<dyn std::error::Error>> {
    application_with_options(
        frontend,
        database_url,
        secure_cookie,
        application_key,
        application_environment,
        application_url,
        ApplicationOptions {
            mailer,
            ..ApplicationOptions::default()
        },
    )
    .await
}

pub async fn application_with_options(
    frontend: Frontend,
    database_url: &str,
    secure_cookie: bool,
    application_key: &[u8],
    application_environment: &str,
    application_url: &str,
    options: ApplicationOptions,
) -> Result<Router, Box<dyn std::error::Error>> {
    let version = frontend.version();
    let database = crate::database::connect(database_url).await?;
    let session_store = SqliteStore::new(database.get_sqlite_connection_pool().clone());
    let session_layer = aurelin_framework::session::web_session_layer(
        session_store,
        "aurelin_session",
        secure_cookie,
    );
    let auth_layer = AuthManagerLayerBuilder::new(
        AuthBackend::new(
            database,
            application_key,
            application_environment,
            application_url,
            options.mailer,
        )
        .await?
        .with_authentication_timebox_duration(options.authentication_timebox_duration),
        session_layer,
    )
    .build();

    Ok(Router::new()
        .merge(crate::routes::http::routes())
        .merge(aurelin_fortify::axum::routes::<Frontend, AuthBackend>())
        .merge(crate::routes::settings::routes())
        .merge(frontend.asset_routes())
        .layer(middleware::from_fn_with_state(
            frontend.ssr(),
            aurelin_framework::view::inertia_ssr::render_document,
        ))
        .layer(VersionLayer::new(version))
        .layer(middleware::from_fn(CsrfMiddleware::middleware))
        .layer(auth_layer)
        .with_state(frontend))
}
