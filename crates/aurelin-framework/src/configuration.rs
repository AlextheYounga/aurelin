//! Application environment loading without process-wide mutation.

use std::{collections::HashMap, env, io, path::Path};

use config::{Config, ConfigError, Environment};
use serde::Deserialize;
use thiserror::Error;

/// Configuration could not be loaded. Values are deliberately not included in
/// dotenv parse diagnostics because environment files may contain credentials.
#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("could not read the application environment file")]
    Read(#[source] io::Error),
    #[error("invalid application environment syntax at character {0}")]
    Syntax(usize),
    #[error("could not parse the application environment")]
    Environment,
    #[error("invalid server configuration: {0}")]
    Server(#[from] ConfigError),
}

/// A snapshot of `.env` plus external variables, with external variables taking
/// precedence. Missing environment files are allowed, as in Laravel.
///
/// An externally supplied `APP_ENV` selects `.env.APP_ENV` when that file exists.
/// No process environment is changed, including when called in an async app.
pub fn environment(base_path: &Path) -> Result<HashMap<String, String>, ConfigurationError> {
    environment_from(base_path, env::vars().collect())
}

/// Loads an environment snapshot from dotenv files and explicit external
/// values. This is useful for hosts that source environment values somewhere
/// other than the current process.
pub fn environment_from(
    base_path: &Path,
    external: HashMap<String, String>,
) -> Result<HashMap<String, String>, ConfigurationError> {
    let mut path = base_path.join(".env");
    if let Some(name) = external.get("APP_ENV") {
        let candidate = base_path.join(format!(".env.{name}"));
        match candidate.try_exists() {
            Ok(true) => path = candidate,
            Ok(false) => {}
            Err(error) => return Err(ConfigurationError::Read(error)),
        }
    }

    let mut values = HashMap::new();
    match dotenvy::from_path_iter(&path) {
        Ok(entries) => {
            for entry in entries {
                let (key, value) = entry.map_err(environment_error)?;
                values.entry(key).or_insert(value);
            }
        }
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(environment_error(error)),
    }
    values.extend(external);
    Ok(values)
}

fn environment_error(error: dotenvy::Error) -> ConfigurationError {
    match error {
        dotenvy::Error::Io(error) => ConfigurationError::Read(error),
        dotenvy::Error::LineParse(_, position) => ConfigurationError::Syntax(position),
        _ => ConfigurationError::Environment,
    }
}

/// HTTP listener settings. Defaults follow Laravel 13's development server.
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl ServerConfig {
    /// Reads `SERVER_HOST` and `SERVER_PORT`, defaulting to `127.0.0.1:8000`.
    pub fn from_environment(values: HashMap<String, String>) -> Result<Self, ConfigurationError> {
        Ok(Config::builder()
            .set_default("host", "127.0.0.1")?
            .set_default("port", 8000)?
            .add_source(Environment::with_prefix("SERVER").source(Some(values)))
            .build()?
            .try_deserialize()?)
    }
}
