use std::{
    error::Error,
    io::{self, IsTerminal},
    path::Path,
};

use aurelin_framework::configuration::{ServerConfig, environment};
use tokio::net::TcpListener;

mod actions;
mod auth;
pub mod bootstrap;
mod database;
mod entities;
pub mod frontend;
pub mod frontend_config;
mod migration;
pub mod password_policy;
pub mod routes;

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_ansi(io::stderr().is_terminal())
        .with_writer(io::stderr)
        .init();
    let environment = environment(Path::new("."))?;
    let config = ServerConfig::from_environment(environment.clone())?;
    let database_url = environment
        .get("DATABASE_URL")
        .map(String::as_str)
        .unwrap_or("sqlite://database.sqlite?mode=rwc");
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
    let frontend = frontend::Frontend::new();
    let _dev_server = frontend.start_dev_server();
    let listener = TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!(address = %listener.local_addr()?, "Aurelin listening");
    let application = bootstrap::application_with_mailer(
        frontend,
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

async fn shutdown() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "could not listen for Ctrl+C");
    }
}
