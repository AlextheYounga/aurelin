use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn framework() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../aurelin-framework")
}

fn create(destination: &Path) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .arg("new")
        .arg(destination)
        .arg("--framework-path")
        .arg(framework())
        .output()
}

#[test]
fn existing_project_is_not_overwritten() -> TestResult {
    let directory = TempDir::new()?;
    let destination = directory.path().join("existing");
    fs::create_dir(&destination)?;
    fs::write(destination.join("Cargo.toml"), "keep this manifest")?;
    fs::write(destination.join(".env"), "keep these secrets")?;
    let result = create(&destination)?;
    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(destination.join("Cargo.toml"))?, "keep this manifest");
    assert_eq!(fs::read_to_string(destination.join(".env"))?, "keep these secrets");
    Ok(())
}

#[test]
fn invalid_cargo_package_name_does_not_create_a_project() -> TestResult {
    let directory = TempDir::new()?;
    let destination = directory.path().join("123-invalid");
    let result = create(&destination)?;
    assert!(!result.status.success());
    assert!(!destination.exists());
    Ok(())
}

#[test]
fn wrong_framework_crate_is_rejected_before_project_creation() -> TestResult {
    let directory = TempDir::new()?;
    let destination = directory.path().join("app");
    let result = Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .arg("new")
        .arg(&destination)
        .arg("--framework-path")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .output()?;
    assert!(!result.status.success());
    assert!(!destination.exists());
    assert!(String::from_utf8(result.stderr)?.contains("must point to the aurelin-framework crate"));
    Ok(())
}

#[test]
fn generated_projects_use_published_aurelin_versions_by_default() -> TestResult {
    let directory = TempDir::new()?;
    let destination = directory.path().join("app");
    let result = Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .args(["new", "--no-interaction"])
        .arg(&destination)
        .args(["--frontend", "react"])
        .output()?;
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let manifest = fs::read_to_string(destination.join("Cargo.toml"))?;
    let version = env!("CARGO_PKG_VERSION");
    assert!(manifest.contains(&format!(r#"aurelin-framework = {{ version = "{version}" }}"#)));
    assert!(manifest.contains(&format!(r#"aurelin-fortify = {{ version = "{version}" }}"#)));
    assert!(!manifest.contains("aurelin-framework = { path"));
    Ok(())
}

#[test]
fn generated_wasm_projects_include_required_hidden_configuration() -> TestResult {
    let directory = TempDir::new()?;
    let destination = directory.path().join("app");
    let result = Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .args(["new", "--no-interaction"])
        .arg(&destination)
        .args(["--frontend", "wasm"])
        .output()?;

    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    for path in [".cargo/config.toml", ".env", ".env.example", ".gitignore"] {
        assert!(destination.join(path).is_file(), "generated project is missing {path}");
    }
    Ok(())
}

#[test]
fn non_interactive_generation_requires_a_destination() -> TestResult {
    let result = Command::new(env!("CARGO_BIN_EXE_aurelin")).args(["new", "--no-interaction"]).output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)?.contains("destination is required"));
    Ok(())
}

#[test]
fn successful_generation_prints_colored_next_steps_when_color_is_enabled() -> TestResult {
    let directory = TempDir::new()?;
    let destination = directory.path().join("app");
    let result = Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .args(["new", "--no-interaction"])
        .arg(&destination)
        .arg("--framework-path")
        .arg(framework())
        .env("CLICOLOR_FORCE", "1")
        .output()?;

    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let output = String::from_utf8(result.stderr)?;
    assert!(output.contains("\u{1b}["));
    assert!(output.contains("Created"));
    assert!(output.contains("Next steps"));
    assert!(output.contains("cargo aurelin serve"));
    Ok(())
}

#[test]
fn help_version_and_completion_commands_work() -> TestResult {
    for arguments in [vec!["--help"], vec!["--version"], vec!["new", "--help"], vec!["completions", "bash"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_aurelin")).args(&arguments).output()?;
        assert!(result.status.success(), "{arguments:?}");
        assert!(!result.stdout.is_empty());
    }
    Ok(())
}
