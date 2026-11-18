use std::{
    error::Error,
    io::{self, IsTerminal},
    path::Path,
};

use kiln_framework::configuration::{ServerConfig, environment};
use tokio::net::TcpListener;

pub mod bootstrap;
pub mod routes;

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_ansi(io::stderr().is_terminal())
        .with_writer(io::stderr)
        .init();
    let config = ServerConfig::from_environment(environment(Path::new("."))?)?;
    let listener = TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!(address = %listener.local_addr()?, "Kiln listening");
    kiln_framework::axum::serve(listener, bootstrap::application())
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "could not listen for Ctrl+C");
    }
}
