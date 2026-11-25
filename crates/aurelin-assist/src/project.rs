use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use aurelin_framework::configuration::environment;
use serde_json::{Value, json};
use toml_edit::DocumentMut;

/// One explicitly selected local application. Environment values are private
/// and are never returned wholesale to an AI client.
#[derive(Clone)]
pub struct Project {
    root: PathBuf,
    environment: HashMap<String, String>,
}

impl Project {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root.canonicalize().context("locate the application directory")?;
        let manifest = fs::read_to_string(root.join("Cargo.toml"))?.parse::<DocumentMut>()?;
        if manifest["package"]["name"].as_str().is_none() {
            bail!("select an application package with --project, not a virtual workspace");
        }
        let environment = environment(&root)?;
        if !matches!(environment.get("APP_ENV").map_or("local", String::as_str), "local" | "testing") {
            bail!("Aurelin Assist is development-only; APP_ENV must be local or testing");
        }
        Ok(Self { root, environment })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve an existing project file. Diagnostic tools cannot follow a
    /// symlink outside the selected application.
    pub fn existing_file(&self, relative: impl AsRef<Path>) -> Result<PathBuf> {
        let path = self.root.join(relative).canonicalize()?;
        if !path.starts_with(&self.root) || !path.is_file() {
            bail!("diagnostic files must be regular files inside the selected application");
        }
        Ok(path)
    }

    pub fn info(&self) -> Result<Value> {
        let manifest = fs::read_to_string(self.existing_file("Cargo.toml")?)?.parse::<DocumentMut>()?;
        let lock_path = self.root.join("Cargo.lock");
        let packages = if lock_path.try_exists()? {
            let lock = fs::read_to_string(self.existing_file("Cargo.lock")?)?.parse::<DocumentMut>()?;
            lock.get("package")
                .and_then(|item| item.as_array_of_tables())
                .map(|packages| {
                    packages
                        .iter()
                        .map(|package| {
                            json!({
                                "name": package["name"].as_str(),
                                "version": package["version"].as_str(),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(json!({
            "name": manifest["package"]["name"].as_str(),
            "root": self.root,
            "packages": packages,
            "package_source": "Cargo.lock (including transitive dependencies; run cargo check if missing)",
            "assist_log_enabled": self.environment.get("ASSIST_LOG").is_some_and(|value| value == "true"),
            "tokio_console_requested": self.environment.get("TOKIO_CONSOLE").is_some_and(|value| value == "true"),
            "database": "SQLite schema inspection is available via an explicit project-relative file path",
        }))
    }
}
