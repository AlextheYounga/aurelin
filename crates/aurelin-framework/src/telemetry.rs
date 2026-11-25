//! Terminal logging and opt-in local JSON diagnostics for Aurelin Assist.
//!
//! Log fields are application-owned and may contain sensitive data. File capture
//! requires `ASSIST_LOG=true` and a local/testing environment. Never log secrets.

use std::{
    collections::HashMap,
    fs::DirBuilder,
    io::{self, IsTerminal},
    path::Path,
};

use thiserror::Error;
use tracing::Subscriber;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::{
    EnvFilter, Layer,
    filter::ParseError,
    fmt,
    layer::SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
};

pub const LOG_DIRECTORY: &str = "storage/logs";
pub const LOG_PREFIX: &str = "aurelin.jsonl";

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("invalid logging filter: {0}")]
    Filter(#[from] ParseError),
    #[error("could not create the local log directory")]
    Directory(#[from] io::Error),
    #[error("could not open the local log file: {0}")]
    File(#[from] InitError),
    #[error("could not install the tracing subscriber: {0}")]
    Subscriber(#[from] TryInitError),
    #[error("Tokio Console requires the tokio-console Cargo feature and a build with RUSTFLAGS='--cfg tokio_unstable'")]
    ConsoleUnavailable,
    #[error("development diagnostics require APP_ENV=local or testing")]
    NonLocal,
}

/// Build a subscriber without setting global state. JSON files rotate daily;
/// at most seven files are retained. Terminal output stays on stderr.
pub fn subscriber(
    root: &Path,
    environment: &HashMap<String, String>,
) -> Result<impl Subscriber + Send + Sync, TelemetryError> {
    let capture = environment.get("ASSIST_LOG").is_some_and(|value| value == "true");
    let console = environment.get("TOKIO_CONSOLE").is_some_and(|value| value == "true");
    let local = matches!(environment.get("APP_ENV").map_or("local", String::as_str), "local" | "testing");
    if (capture || console) && !local {
        return Err(TelemetryError::NonLocal);
    }
    let filter = EnvFilter::try_new(environment.get("RUST_LOG").map_or("info", String::as_str))?;
    let file = if capture {
        let directory = root.join(LOG_DIRECTORY);
        let mut builder = DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        Some(
            RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix(LOG_PREFIX)
                .max_log_files(7)
                .build(directory)?,
        )
    } else {
        None
    };
    #[cfg(feature = "tokio-console")]
    let console_layer = if console {
        if !cfg!(tokio_unstable) {
            return Err(TelemetryError::ConsoleUnavailable);
        }
        Some(console_subscriber::ConsoleLayer::builder().server_addr(([127, 0, 0, 1], 6669)).spawn())
    } else {
        None
    };
    #[cfg(not(feature = "tokio-console"))]
    if console {
        return Err(TelemetryError::ConsoleUnavailable);
    }
    let subscriber = tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(io::stderr().is_terminal()).with_writer(io::stderr).with_filter(filter.clone()))
        .with(file.map(|writer| fmt::layer().json().with_ansi(false).with_writer(writer).with_filter(filter)));
    #[cfg(feature = "tokio-console")]
    let subscriber = subscriber.with(console_layer);
    Ok(subscriber)
}

/// Install the application's single process-wide subscriber.
pub fn init(root: &Path, environment: &HashMap<String, String>) -> Result<(), TelemetryError> {
    subscriber(root, environment)?.try_init()?;
    Ok(())
}
