use std::{
    error::Error,
    io::{self, IsTerminal},
    path::Path,
};

use aurelin_framework::configuration::{ServerConfig, environment};
use tokio::net::TcpListener;

pub mod bootstrap;
pub mod console;
pub mod database;
pub mod jobs;
#[path = "../database/migrations/mod.rs"]
mod migrations;
pub mod routes;

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_ansi(io::stderr().is_terminal())
        .with_writer(io::stderr)
        .init();
    let environment = environment(Path::new("."))?;
    console::run(environment).await
}

async fn serve(
    environment: std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    let config = ServerConfig::from_environment(environment)?;
    let listener = TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!(address = %listener.local_addr()?, "Aurelin listening");
    aurelin_framework::axum::serve(listener, bootstrap::application())
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "could not listen for Ctrl+C");
    }
}
