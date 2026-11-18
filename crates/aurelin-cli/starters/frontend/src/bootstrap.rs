use aurelin_framework::{
    axum::{Router, middleware},
    inertia::axum::VersionLayer,
};
use axum_login::AuthManagerLayerBuilder;
use axum_tower_sessions_csrf::CsrfMiddleware;
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::SqliteStore;

use crate::{auth::AuthBackend, frontend::Frontend};

/// Compose routes, application state, and middleware here.
pub async fn application(
    frontend: Frontend,
    database_url: &str,
    secure_cookie: bool,
) -> Result<Router, Box<dyn std::error::Error>> {
    let version = frontend.version();
    let database = crate::database::connect(database_url).await?;
    let session_store = SqliteStore::new(database.get_sqlite_connection_pool().clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("aurelin_session")
        .with_secure(secure_cookie);
    let auth_layer =
        AuthManagerLayerBuilder::new(AuthBackend::new(database).await?, session_layer).build();

    Ok(Router::new()
        .merge(crate::routes::http::routes())
        .merge(crate::routes::authentication::routes())
        .merge(crate::routes::email_verification::routes())
        .merge(crate::routes::password_reset::routes())
        .merge(crate::routes::password_confirmation::routes())
        .nest("/build", frontend.asset_routes())
        .layer(VersionLayer::new(version))
        .layer(middleware::from_fn(CsrfMiddleware::middleware))
        .layer(auth_layer)
        .with_state(frontend))
}
