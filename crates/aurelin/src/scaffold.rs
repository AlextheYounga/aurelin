use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use include_dir::{Dir, include_dir};
use rand::{Rng, distr::Alphanumeric};
use toml_edit::{DocumentMut, InlineTable, value};

static STARTER: Dir<'_> = include_dir!("$OUT_DIR/starters/base");
static AUTHENTICATION: Dir<'_> = include_dir!("$OUT_DIR/starters/authentication");
static FRONTEND: Dir<'_> = include_dir!("$OUT_DIR/starters/frontend");
static REACT: Dir<'_> = include_dir!("$OUT_DIR/starters/react");
static VUE: Dir<'_> = include_dir!("$OUT_DIR/starters/vue");
static WASM: Dir<'_> = include_dir!("$OUT_DIR/starters/wasm");

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Frontend {
    React,
    Vue,
    Wasm,
}

#[derive(Debug)]
pub enum FrameworkSource {
    Registry,
    Path(PathBuf),
}

impl FrameworkSource {
    pub fn resolve(path: Option<PathBuf>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::Registry);
        };
        let framework = path.canonicalize().context("locate the aurelin-framework crate")?;
        let manifest = fs::read_to_string(framework.join("Cargo.toml"))
            .context("read the aurelin-framework manifest")?
            .parse::<DocumentMut>()
            .context("parse the aurelin-framework manifest")?;
        if manifest["package"]["name"].as_str() != Some("aurelin-framework") {
            bail!("--framework-path must point to the aurelin-framework crate");
        }
        Ok(Self::Path(framework))
    }
}

pub fn create(path: &Path, framework: &FrameworkSource, name: Option<&str>, frontend: Option<Frontend>) -> Result<()> {
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

fn install(path: &Path, framework: &FrameworkSource, frontend: Option<Frontend>) -> Result<()> {
    let manifest_path = path.join("Cargo.toml");
    let mut manifest = fs::read_to_string(&manifest_path)?.parse::<DocumentMut>()?;
    let crate_name = manifest["package"]["name"].as_str().context("generated package has no name")?.replace('-', "_");
    let dependencies = &mut manifest["dependencies"];
    dependencies["aurelin-framework"] = value(aurelin_dependency(framework, "aurelin-framework")?);
    if frontend.is_some_and(Frontend::uses_authentication) {
        dependencies["aurelin-fortify"] = value(aurelin_dependency(framework, "aurelin-fortify")?);
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
    dependencies["clap"] = value({
        let mut dependency = InlineTable::new();
        dependency.insert("version", "4".into());
        dependency.insert("features", ["derive"].into_iter().collect::<toml_edit::Array>().into());
        dependency
    });
    dependencies["serde_json"] = value("1");
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
    if frontend.is_some_and(Frontend::uses_authentication) {
        dependencies["axum-login"] = value("0.18");
        dependencies["axum-tower-sessions-csrf"] = value("=0.1.1");
        dependencies["axum-valid"] = value("0.25");
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
    }
    if frontend.is_some_and(Frontend::uses_inertia) {
        dependencies["vite-rs"] = value({
            let mut dependency = InlineTable::new();
            dependency.insert("version", "0.3".into());
            dependency.insert("default-features", false.into());
            dependency.insert("features", ["content-hash"].into_iter().collect::<toml_edit::Array>().into());
            dependency
        });
        dependencies["vite-rs-axum-0-8"] = value("0.3");
    }
    if frontend.is_some_and(Frontend::uses_authentication) {
        manifest["dev-dependencies"]["totp-rs"] = value("6.0");
    }
    if frontend == Some(Frontend::Wasm) {
        install_wasm_manifest(&mut manifest)?;
    }
    let mut tower = InlineTable::new();
    tower.insert("version", "0.5".into());
    tower.insert("features", ["util"].into_iter().collect::<toml_edit::Array>().into());
    manifest["dev-dependencies"]["tower"] = value(tower);
    manifest["dev-dependencies"]["tempfile"] = value("3");
    fs::write(manifest_path, manifest.to_string())?;
    STARTER.extract(path).context("extract the application starter")?;
    let database_directory = path.join("database");
    fs::create_dir_all(&database_directory)?;
    fs::write(database_directory.join("database.sqlite"), [])?;
    if let Some(frontend) = frontend {
        match frontend {
            Frontend::React => {
                AUTHENTICATION.extract(path).context("extract the authentication starter")?;
                FRONTEND.extract(path).context("extract the Inertia starter")?;
                REACT.extract(path).context("extract the React starter")?;
            }
            Frontend::Vue => {
                AUTHENTICATION.extract(path).context("extract the authentication starter")?;
                FRONTEND.extract(path).context("extract the Inertia starter")?;
                VUE.extract(path).context("extract the Vue starter")?;
            }
            Frontend::Wasm => {
                AUTHENTICATION.extract(path).context("extract the authentication starter")?;
                WASM.extract(path).context("extract the WASM starter")?;
            }
        }
    }
    for source in [
        path.join("src/main.rs"),
        path.join("tests/application.rs"),
        path.join("tests/authentication.rs"),
        path.join("tests/email_verification.rs"),
        path.join("tests/jobs.rs"),
        path.join("tests/mail_delivery.rs"),
        path.join("tests/password_reset.rs"),
        path.join("tests/password_policy.rs"),
        path.join("tests/password_rehash.rs"),
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
    let mut environment = fs::read_to_string(path.join(".env.example")).context("read generated .env.example")?;
    if frontend.is_some_and(Frontend::uses_authentication) {
        let application_key: String = rand::rng().sample_iter(Alphanumeric).take(64).map(char::from).collect();
        environment = environment.replace("change-this-to-a-long-random-secret", &application_key);
    }
    fs::write(path.join(".env"), environment)?;
    Ok(())
}

impl Frontend {
    fn uses_authentication(self) -> bool {
        matches!(self, Self::React | Self::Vue | Self::Wasm)
    }

    fn uses_inertia(self) -> bool {
        matches!(self, Self::React | Self::Vue)
    }
}

fn install_wasm_manifest(manifest: &mut DocumentMut) -> Result<()> {
    manifest["lib"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["features"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["package"]["metadata"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["package"]["metadata"]["leptos"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["profile"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["profile"]["wasm-release"] = toml_edit::Item::Table(toml_edit::Table::new());
    manifest["lib"]["crate-type"] = string_array(&["cdylib", "rlib"]);

    let dependencies = &mut manifest["dependencies"];
    let framework = dependencies["aurelin-framework"]
        .as_inline_table_mut()
        .context("generated aurelin-framework dependency is not an inline table")?;
    framework.insert("features", ["leptos"].into_iter().collect::<toml_edit::Array>().into());
    dependencies["leptos"] = value(dependency_with_features("0.8.15", &[]));
    dependencies["leptos_axum"] = value(optional_dependency("0.8.8"));
    dependencies["leptos_meta"] = value(dependency_with_features("0.8.6", &[]));
    dependencies["leptos_router"] = value(dependency_with_features("0.8.14", &[]));
    dependencies["console_error_panic_hook"] = value(optional_dependency("0.1"));
    dependencies["wasm-bindgen"] = value(optional_dependency("0.2.106"));

    let server_dependencies = [
        "aurelin-framework",
        "aurelin-fortify",
        "tokio",
        "tracing",
        "tracing-subscriber",
        "clap",
        "serde_json",
        "serde",
        "sea-orm",
        "sea-orm-migration",
        "axum-login",
        "axum-tower-sessions-csrf",
        "axum-valid",
        "thiserror",
        "tower-sessions",
        "tower-sessions-sqlx-store",
        "urlencoding",
        "validator",
    ];
    for dependency in server_dependencies {
        make_optional(dependencies, dependency);
    }

    manifest["features"]["default"] = string_array(&["ssr"]);
    manifest["features"]["hydrate"] =
        string_array(&["leptos/hydrate", "dep:console_error_panic_hook", "dep:wasm-bindgen"]);
    manifest["features"]["ssr"] = string_array(&[
        "dep:aurelin-framework",
        "dep:aurelin-fortify",
        "dep:tokio",
        "dep:tracing",
        "dep:tracing-subscriber",
        "dep:clap",
        "dep:serde_json",
        "dep:serde",
        "dep:sea-orm",
        "dep:sea-orm-migration",
        "dep:axum-login",
        "dep:axum-tower-sessions-csrf",
        "dep:axum-valid",
        "dep:thiserror",
        "dep:tower-sessions",
        "dep:tower-sessions-sqlx-store",
        "dep:urlencoding",
        "dep:validator",
        "dep:leptos_axum",
        "leptos/ssr",
        "leptos_meta/ssr",
        "leptos_router/ssr",
    ]);

    let package_name = manifest["package"]["name"].as_str().unwrap_or("aurelin-app").to_owned();
    let leptos = &mut manifest["package"]["metadata"]["leptos"];
    leptos["output-name"] = value(package_name);
    leptos["site-root"] = value("target/site");
    leptos["site-pkg-dir"] = value("pkg");
    leptos["assets-dir"] = value("public");
    leptos["site-addr"] = value("127.0.0.1:8000");
    leptos["reload-port"] = value(8001);
    leptos["bin-features"] = string_array(&["ssr"]);
    leptos["bin-default-features"] = value(false);
    leptos["lib-features"] = string_array(&["hydrate"]);
    leptos["lib-default-features"] = value(false);
    leptos["lib-profile-release"] = value("wasm-release");

    let wasm_release = &mut manifest["profile"]["wasm-release"];
    wasm_release["inherits"] = value("release");
    wasm_release["opt-level"] = value("z");
    wasm_release["lto"] = value(true);
    wasm_release["codegen-units"] = value(1);
    wasm_release["panic"] = value("abort");
    Ok(())
}

fn dependency_with_features(version: &str, features: &[&str]) -> InlineTable {
    let mut dependency = InlineTable::new();
    dependency.insert("version", version.into());
    if !features.is_empty() {
        dependency.insert("features", features.iter().copied().collect::<toml_edit::Array>().into());
    }
    dependency
}

fn optional_dependency(version: &str) -> InlineTable {
    let mut dependency = dependency_with_features(version, &[]);
    dependency.insert("optional", true.into());
    dependency
}

fn make_optional(dependencies: &mut toml_edit::Item, name: &str) {
    if let Some(dependency) = dependencies[name].as_inline_table_mut() {
        dependency.insert("optional", true.into());
        return;
    }
    if let Some(version) = dependencies[name].as_str().map(str::to_owned) {
        dependencies[name] = value(optional_dependency(&version));
    }
}

fn string_array(values: &[&str]) -> toml_edit::Item {
    value(values.iter().copied().collect::<toml_edit::Array>())
}

fn aurelin_dependency(source: &FrameworkSource, package: &str) -> Result<InlineTable> {
    let mut dependency = InlineTable::new();
    dependency.insert("version", env!("CARGO_PKG_VERSION").into());
    if let FrameworkSource::Path(framework) = source {
        let path = if package == "aurelin-framework" {
            framework.clone()
        } else {
            framework.parent().context("framework crate must have a parent directory")?.join(package)
        };
        dependency.insert("path", path.to_str().context("Aurelin crate path must be valid Unicode")?.into());
    }
    Ok(dependency)
}
