use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;

fn framework() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../aurelin-framework")
}

#[test]
fn react_and_vue_projects_compile_and_test_their_inertia_boundaries() -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/generated-frontends");
    for frontend in ["react", "vue"] {
        let destination = directory.path().join(frontend);
        let create = Command::new(env!("CARGO_BIN_EXE_aurelin"))
            .arg("new")
            .arg(&destination)
            .args(["--name", &format!("aurelin-{frontend}-test"), "--framework-path"])
            .arg(framework())
            .args(["--frontend", frontend])
            .output()?;
        assert!(create.status.success(), "{}", String::from_utf8_lossy(&create.stderr));
        let environment = fs::read_to_string(destination.join(".env"))?;
        assert!(environment.contains("APP_KEY="));
        assert!(!environment.contains("change-this-to-a-long-random-secret"));
        assert!(environment.contains("DATABASE_URL=sqlite://database/database.sqlite?mode=rwc"));
        assert!(destination.join("database/database.sqlite").is_file());
        assert!(destination.join("database/migrations/mod.rs").is_file());
        assert!(!destination.join("src/migration").exists());
        assert!(destination.join("src/config/frontend.rs").is_file());
        assert!(destination.join("src/config/password_policy.rs").is_file());
        assert!(!destination.join("src/frontend_config.rs").exists());
        assert!(!destination.join("src/password_policy.rs").exists());

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
        let commands =
            Command::new("cargo").arg("aurelin").env("CARGO_TARGET_DIR", &target).current_dir(&destination).output()?;
        assert!(commands.status.success(), "{}", String::from_utf8_lossy(&commands.stderr));
        let commands = String::from_utf8(commands.stdout)?;
        for command in [
            "dev",
            "serve",
            "migrate",
            "migrate:status",
            "migrate:rollback",
            "migrate:fresh",
            "db:seed",
            "queue:work",
            "queue:failed",
            "queue:retry",
            "queue:forget",
            "queue:flush",
        ] {
            assert!(commands.contains(command), "missing `{command}` from:\n{commands}");
        }

        let unmigrated_server = Command::new("cargo")
            .args(["aurelin", "serve"])
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(!unmigrated_server.status.success());
        assert!(String::from_utf8(unmigrated_server.stderr)?.contains("cargo aurelin migrate"));

        let migrated = Command::new("cargo")
            .args(["aurelin", "migrate"])
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(migrated.status.success(), "{}", String::from_utf8_lossy(&migrated.stderr));
        assert!(String::from_utf8(migrated.stdout)?.contains("Applied"));

        let migration_status = Command::new("cargo")
            .args(["aurelin", "migrate:status"])
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(migration_status.status.success(), "{}", String::from_utf8_lossy(&migration_status.stderr));
        let migration_status = String::from_utf8(migration_status.stdout)?;
        assert!(migration_status.contains("Applied"));
        assert!(migration_status.contains("m20261118_000001_create_auth_tables"));
        assert!(migration_status.contains("m20261125_000001_create_posts"));

        let denied = Command::new("cargo")
            .args(["aurelin", "db:seed"])
            .env("APP_ENV", "production")
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(!denied.status.success());
        assert!(String::from_utf8_lossy(&denied.stderr).contains("without --force"));

        let forced = Command::new("cargo")
            .args(["aurelin", "db:seed", "--force"])
            .env("APP_ENV", "production")
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(&destination)
            .output()?;
        assert!(!forced.status.success());
        assert!(String::from_utf8_lossy(&forced.stderr).contains("requires an existing user"));

        assert!(destination.join("package-lock.json").is_file());
        let vite_config = fs::read_to_string(destination.join("vite.config.ts"))?;
        assert!(vite_config.contains("base: command === 'build' ? '/build/' : '',"));
    }
    Ok(())
}
