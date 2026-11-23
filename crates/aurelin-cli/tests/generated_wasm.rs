use std::{error::Error, fs, path::Path, process::Command};

use tempfile::TempDir;

#[test]
fn generated_wasm_project_compiles_for_server_and_browser() -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let destination = directory.path().join("wasm");
    let cli_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let create = Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .arg("new")
        .arg(&destination)
        .args(["--name", "aurelin-wasm-test", "--framework-path"])
        .arg(cli_root.join("../aurelin-framework"))
        .args(["--frontend", "wasm"])
        .output()?;
    assert!(create.status.success(), "{}", String::from_utf8_lossy(&create.stderr));

    assert!(!destination.join("package.json").exists());
    assert!(!destination.join("vite.config.ts").exists());
    assert!(fs::read_to_string(destination.join(".env"))?.contains("APP_KEY="));
    let manifest = fs::read_to_string(destination.join("Cargo.toml"))?;
    assert!(manifest.contains("leptos_axum"));
    assert!(!manifest.contains("vite-rs"));

    let target = cli_root.join("../../target/generated-wasm");
    for arguments in [
        vec!["fmt", "--all", "--", "--check"],
        vec!["check"],
        vec!["clippy", "--all-targets", "--", "-D", "warnings"],
        vec!["check", "--lib", "--no-default-features", "--features", "hydrate", "--target", "wasm32-unknown-unknown"],
        vec!["test"],
    ] {
        let output = Command::new("cargo")
            .args(arguments)
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
