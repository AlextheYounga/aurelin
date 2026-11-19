use std::{fs, path::Path, process::Command};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use include_dir::{Dir, include_dir};
use rand::{Rng, distr::Alphanumeric};
use toml_edit::{DocumentMut, InlineTable, value};

static STARTER: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/starters/base");
static FRONTEND: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/starters/frontend");
static REACT: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/starters/react");
static VUE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/starters/vue");

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Frontend {
    React,
    Vue,
}

pub fn create(path: &Path, framework_path: &Path, name: Option<&str>, frontend: Option<Frontend>) -> Result<()> {
    let framework = framework_path.canonicalize().context("locate the aurelin-framework crate")?;
    let manifest = fs::read_to_string(framework.join("Cargo.toml"))
        .context("read the aurelin-framework manifest")?
        .parse::<DocumentMut>()
        .context("parse the aurelin-framework manifest")?;
    if manifest["package"]["name"].as_str() != Some("aurelin-framework") {
        bail!("--framework-path must point to the aurelin-framework crate");
    }
    let framework = framework.to_str().context("framework path must be valid Unicode")?;

    // Cargo owns project-name validation and refuses existing destinations.
    // Never use `cargo init`, which would adopt an existing directory.
    let mut cargo = Command::new("cargo");
    cargo.args(["new", "--bin", "--vcs", "none"]);
    if let Some(name) = name {
        cargo.args(["--name", name]);
    }
    let status = cargo.arg("--").arg(path).status().context("run `cargo new`")?;
    if !status.success() {
        bail!("`cargo new` failed; no starter files were written");
    }

    install(path, framework, frontend).with_context(|| {
        format!("could not finish {} (partial project retained; no files were deleted)", path.display())
    })
}

fn install(path: &Path, framework: &str, frontend: Option<Frontend>) -> Result<()> {
    let manifest_path = path.join("Cargo.toml");
    let mut manifest = fs::read_to_string(&manifest_path)?.parse::<DocumentMut>()?;
    let crate_name = manifest["package"]["name"].as_str().context("generated package has no name")?.replace('-', "_");
    let dependencies = &mut manifest["dependencies"];
    let mut aurelin = InlineTable::new();
    aurelin.insert("path", framework.into());
    dependencies["aurelin-framework"] = value(aurelin);
    if frontend.is_some() {
        let fortify_path = Path::new(framework)
            .parent()
            .context("framework crate must have a parent directory")?
            .join("aurelin-fortify");
        let mut fortify = InlineTable::new();
        fortify.insert("path", fortify_path.to_str().context("aurelin-fortify path must be valid Unicode")?.into());
        dependencies["aurelin-fortify"] = value(fortify);
    }
    let mut tokio = InlineTable::new();
    tokio.insert("version", "1".into());
    tokio.insert(
        "features",
        ["macros", "rt-multi-thread", "signal", "sync"].into_iter().collect::<toml_edit::Array>().into(),
    );
    dependencies["tokio"] = value(tokio);
    dependencies["tracing"] = value("0.1");
    dependencies["tracing-subscriber"] = value("0.3");
    dependencies["serde_json"] = value("1");
    if frontend.is_some() {
        dependencies["axum-login"] = value("0.18");
        dependencies["axum-tower-sessions-csrf"] = value("=0.1.1");
        dependencies["axum-valid"] = value("0.25");
        dependencies["lettre"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "0.11.19".into());
            dependency.insert("default-features", false.into());
            dependency.insert(
                "features",
                ["builder", "smtp-transport", "tokio1-rustls-tls"].into_iter().collect::<toml_edit::Array>().into(),
            );
            dependency
        });
        dependencies["serde"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "1".into());
            dependency.insert("features", ["derive"].into_iter().collect::<toml_edit::Array>().into());
            dependency
        });
        dependencies["sea-orm"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "1.1".into());
            dependency.insert("default-features", false.into());
            dependency.insert(
                "features",
                ["macros", "runtime-tokio-rustls", "sqlx-sqlite", "sqlite-use-returning-for-3_35"]
                    .into_iter()
                    .collect::<toml_edit::Array>()
                    .into(),
            );
            dependency
        });
        dependencies["sea-orm-migration"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "1.1".into());
            dependency.insert("default-features", false.into());
            dependency.insert(
                "features",
                ["runtime-tokio-rustls", "sqlx-sqlite"].into_iter().collect::<toml_edit::Array>().into(),
            );
            dependency
        });
        dependencies["thiserror"] = value("2");
        dependencies["tower-sessions"] = value("0.14");
        dependencies["tower-sessions-sqlx-store"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "=0.15.0".into());
            dependency.insert("features", ["sqlite"].into_iter().collect::<toml_edit::Array>().into());
            dependency
        });
        dependencies["urlencoding"] = value("2");
        dependencies["validator"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "0.20".into());
            dependency.insert("features", ["derive"].into_iter().collect::<toml_edit::Array>().into());
            dependency
        });
        dependencies["vite-rs"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "0.3".into());
            dependency.insert("default-features", false.into());
            dependency.insert("features", ["content-hash"].into_iter().collect::<toml_edit::Array>().into());
            dependency
        });
        dependencies["vite-rs-axum-0-8"] = value("0.3");
        manifest["dev-dependencies"]["totp-rs"] = value("6.0");
    }
    let mut tower = InlineTable::new();
    tower.insert("version", "0.5".into());
    tower.insert("features", ["util"].into_iter().collect::<toml_edit::Array>().into());
    manifest["dev-dependencies"]["tower"] = value(tower);
    fs::write(manifest_path, manifest.to_string())?;
    STARTER.extract(path).context("extract the application starter")?;
    if let Some(frontend) = frontend {
        FRONTEND.extract(path).context("extract the Inertia starter")?;
        match frontend {
            Frontend::React => REACT.extract(path).context("extract the React starter")?,
            Frontend::Vue => VUE.extract(path).context("extract the Vue starter")?,
        }
    }
    for source in [
        path.join("src/main.rs"),
        path.join("tests/application.rs"),
        path.join("tests/authentication.rs"),
        path.join("tests/email_verification.rs"),
        path.join("tests/mail_delivery.rs"),
        path.join("tests/password_reset.rs"),
        path.join("tests/password_policy.rs"),
        path.join("tests/remember_me.rs"),
        path.join("tests/passkeys.rs"),
        path.join("tests/settings.rs"),
        path.join("tests/two_factor_authentication.rs"),
    ] {
        if !source.exists() {
            continue;
        }
        let contents = fs::read_to_string(&source)?;
        fs::write(source, contents.replace("__AURELIN_CRATE__", &crate_name))?;
    }
    let mut environment = fs::read_to_string(path.join(".env.example"))?;
    if frontend.is_some() {
        let application_key: String = rand::rng().sample_iter(Alphanumeric).take(64).map(char::from).collect();
        environment = environment.replace("change-this-to-a-long-random-secret", &application_key);
    }
    fs::write(path.join(".env"), environment)?;
    Ok(())
}
