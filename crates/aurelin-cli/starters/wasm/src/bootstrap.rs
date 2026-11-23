use std::error::Error;

use aurelin_framework::{axum::Router, mail::Mailer};
use axum_login::AuthManagerLayerBuilder;
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};
use tower_sessions_sqlx_store::SqliteStore;

use crate::app::{App, shell};
use crate::auth::AuthBackend;

/// Compose Leptos routes, application state, and middleware here.
pub async fn application(
    options: LeptosOptions,
    database_url: &str,
    secure_cookie: bool,
    application_key: &[u8],
    application_environment: &str,
    application_url: &str,
    mailer: Mailer,
) -> Result<Router, Box<dyn Error>> {
    let routes = generate_route_list(App);
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
            mailer,
        )
        .await?,
        session_layer,
    )
    .build();

    Ok(Router::new()
        .merge(crate::routes::http::routes())
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(auth_layer)
        .with_state(options))
}
