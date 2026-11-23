pub mod app;

#[cfg(feature = "ssr")]
use std::{
    error::Error,
    io::{self, IsTerminal},
    path::Path,
};

#[cfg(feature = "ssr")]
use aurelin_framework::configuration::{ServerConfig, environment};
#[cfg(feature = "ssr")]
use tokio::net::TcpListener;

#[cfg(feature = "ssr")]
mod actions;
#[cfg(feature = "ssr")]
pub mod auth;
#[cfg(feature = "ssr")]
pub mod bootstrap;
#[cfg(feature = "ssr")]
pub mod config;
#[cfg(feature = "ssr")]
pub mod console;
#[cfg(feature = "ssr")]
mod database;
#[cfg(feature = "ssr")]
mod entities;
#[cfg(feature = "ssr")]
pub mod jobs;
#[cfg(feature = "ssr")]
#[path = "../database/migrations/mod.rs"]
mod migrations;
#[cfg(feature = "ssr")]
pub mod routes;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
}

#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_ansi(io::stderr().is_terminal())
        .with_writer(io::stderr)
        .init();
    let environment = environment(Path::new("."))?;
    console::run(environment).await
}

#[cfg(feature = "ssr")]
async fn serve(
    environment: std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    let server = ServerConfig::from_environment(environment.clone())?;
    let database_url = environment
        .get("DATABASE_URL")
        .map(String::as_str)
        .unwrap_or("sqlite://database/database.sqlite?mode=rwc");
    let secure_cookie = environment
        .get("SESSION_SECURE_COOKIE")
        .map(String::as_str)
        .unwrap_or("false")
        .parse::<bool>()?;
    let application_key = environment.get("APP_KEY").ok_or("APP_KEY must be set")?;
    let application_environment = environment
        .get("APP_ENV")
        .map(String::as_str)
        .unwrap_or("local");
    let application_url = environment
        .get("APP_URL")
        .map(String::as_str)
        .unwrap_or("http://localhost:8000");
    let mailer = aurelin_framework::mail::Mailer::from_environment(&environment)?;
    let mut leptos = leptos::config::get_configuration(None)?.leptos_options;
    leptos.site_addr.set_ip(server.host.parse()?);
    leptos.site_addr.set_port(server.port);

    let listener = TcpListener::bind(leptos.site_addr).await?;
    tracing::info!(address = %listener.local_addr()?, "Aurelin listening");
    let application = bootstrap::application(
        leptos,
        database_url,
        secure_cookie,
        application_key.as_bytes(),
        application_environment,
        application_url,
        mailer,
    )
    .await?;
    aurelin_framework::axum::serve(
        listener,
        application.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    Ok(())
}

#[cfg(feature = "ssr")]
async fn shutdown() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "could not listen for Ctrl+C");
    }
}
