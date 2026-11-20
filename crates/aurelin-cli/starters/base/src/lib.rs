use std::{
    error::Error,
    io::{self, IsTerminal},
    path::Path,
};

use aurelin_framework::configuration::{ServerConfig, environment};
use tokio::net::TcpListener;

pub mod bootstrap;
pub mod jobs;
pub mod routes;

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_ansi(io::stderr().is_terminal())
        .with_writer(io::stderr)
        .init();
    let environment = environment(Path::new("."))?;
    let database_url = environment
        .get("DATABASE_URL")
        .map(String::as_str)
        .unwrap_or("sqlite://database.sqlite?mode=rwc");
    match std::env::args().nth(1).as_deref() {
        Some("worker") => return jobs::work(database_url).await,
        Some(command) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown command `{command}`; expected `worker`"),
            )
            .into());
        }
        None => {}
    }
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
