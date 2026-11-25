use std::{error::Error, path::Path, process::Command};

#[test]
fn generated_api_runs_its_real_persistence_and_http_tests() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::TempDir::new()?;
    let app = directory.path().join("app");
    let framework = Path::new(env!("CARGO_MANIFEST_DIR")).join("../aurelin-framework");
    let output = Command::new(env!("CARGO_BIN_EXE_aurelin"))
        .args(["new", "--no-interaction", "--frontend", "react", "--api", "--name", "api-test", "--framework-path"])
        .arg(framework)
        .arg(&app)
        .output()?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/generated-frontends");
    let output = Command::new("cargo")
        .args(["test", "--test", "api"])
        .env("CARGO_TARGET_DIR", target)
        .current_dir(app)
        .output()?;
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
