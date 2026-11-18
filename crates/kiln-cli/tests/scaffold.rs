use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn framework() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../kiln-framework")
}

fn create(destination: &Path) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
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
    let result = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("new")
        .arg(&destination)
        .arg("--framework-path")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .output()?;
    assert!(!result.status.success());
    assert!(!destination.exists());
    assert!(String::from_utf8(result.stderr)?.contains("must point to the kiln-framework crate"));
    Ok(())
}

#[test]
fn help_version_and_completion_commands_work() -> TestResult {
    for arguments in [vec!["--help"], vec!["--version"], vec!["new", "--help"], vec!["completions", "bash"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_kiln")).args(&arguments).output()?;
        assert!(result.status.success(), "{arguments:?}");
        assert!(!result.stdout.is_empty());
    }
    Ok(())
}
