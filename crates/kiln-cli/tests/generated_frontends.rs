use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;

fn framework() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../kiln-framework")
}

#[test]
fn react_and_vue_projects_compile_and_test_their_inertia_boundaries() -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/generated-frontends");
    for frontend in ["react", "vue"] {
        let destination = directory.path().join(frontend);
        let create = Command::new(env!("CARGO_BIN_EXE_kiln"))
            .arg("new")
            .arg(&destination)
            .args(["--name", &format!("kiln-{frontend}-test"), "--framework-path"])
            .arg(framework())
            .args(["--frontend", frontend])
            .output()?;
        assert!(create.status.success(), "{}", String::from_utf8_lossy(&create.stderr));

        for arguments in [
            vec!["fmt", "--all", "--", "--check"],
            vec!["clippy", "--all-targets", "--", "-D", "warnings"],
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
        assert!(destination.join("package-lock.json").is_file());
        assert!(destination.join("vite.config.ts").is_file());
    }
    Ok(())
}
