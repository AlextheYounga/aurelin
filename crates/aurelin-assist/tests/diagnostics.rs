mod common;

use std::{fs, path::Path};

use anyhow::Result;
use aurelin_assist::{logs, project::Project, schema};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use serde_json::json;

#[test]
fn project_context_reports_locked_versions_without_environment_credentials() -> Result<()> {
    let directory = common::application()?;
    fs::write(directory.path().join("Cargo.lock"), "version = 4\n[[package]]\nname = 'axum'\nversion = '0.8.9'\n")?;
    let project = Project::open(directory.path())?;
    let info = project.info()?;
    assert_eq!(info["name"], "example-app");
    assert_eq!(info["packages"], json!([{"name": "axum", "version": "0.8.9"}]));
    assert!(!info.to_string().contains("must-not-leak"));
    let output = common::cli(directory.path(), &["doctor"])?;
    assert!(output.status.success());
    assert!(!String::from_utf8(output.stdout)?.contains("must-not-leak"));
    Ok(())
}

#[test]
fn recent_logs_and_last_error_span_daily_rotation_and_ignore_incomplete_records() -> Result<()> {
    let directory = common::application()?;
    let logs_directory = directory.path().join("storage/logs");
    fs::create_dir_all(&logs_directory)?;
    fs::write(
        logs_directory.join("aurelin.jsonl.2026-11-24"),
        "{\"level\":\"ERROR\",\"fields\":{\"message\":\"old error\"}}\n",
    )?;
    fs::write(
        logs_directory.join("aurelin.jsonl.2026-11-25"),
        "{\"level\":\"INFO\",\"fields\":{\"message\":\"started\"}}\nnot-json\n{\"level\":\"ERROR\",\"fields\":{\"message\":\"latest error\"}}\n{\"level\":\"ERROR\"}",
    )?;
    let project = Project::open(directory.path())?;
    let records = logs::recent(&project, 2, false)?;
    assert_eq!(records[0]["fields"]["message"], "started");
    assert_eq!(records[1]["fields"]["message"], "latest error");
    assert_eq!(logs::recent(&project, 1, true)?[0]["fields"]["message"], "latest error");
    assert!(logs::recent(&project, 101, false).is_err());
    assert!(logs::recent(&project, 0, false).is_err());
    Ok(())
}

#[test]
fn logs_bound_the_read_window_and_skip_a_partial_record_at_its_start() -> Result<()> {
    let directory = common::application()?;
    let root = directory.path().join("storage/logs");
    fs::create_dir_all(&root)?;
    let old = json!({"level": "ERROR", "message": "outside window"});
    let latest = json!({"level": "ERROR", "message": "inside window"});
    fs::write(root.join("aurelin.jsonl.2026-11-25"), format!("{old}\n{}\n{latest}\n", "x".repeat(1024 * 1024)))?;
    let project = Project::open(directory.path())?;
    assert_eq!(logs::recent(&project, 100, false)?, vec![latest]);
    Ok(())
}

#[tokio::test]
async fn sqlite_inspection_returns_metadata_not_rows_and_never_creates_missing_databases() -> Result<()> {
    let directory = common::application()?;
    let path = directory.path().join("database with spaces.sqlite");
    let url = url::Url::from_file_path(&path).map_err(|()| anyhow::anyhow!("fixture path"))?;
    let database = Database::connect(format!("sqlite://{}?mode=rwc", url.path())).await?;
    database
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, password_hash TEXT NOT NULL)".to_owned(),
        ))
        .await?;
    database
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO users (password_hash) VALUES ('private-password-hash')".to_owned(),
        ))
        .await?;
    database.close().await?;
    let before = fs::read(&path)?;
    let project = Project::open(directory.path())?;
    let schema = schema::inspect(&project, Path::new("database with spaces.sqlite")).await?;
    assert_eq!(schema["tables"][0]["name"], "users");
    assert_eq!(schema["tables"][0]["columns"][1]["name"], "password_hash");
    assert_eq!(schema["tables"][0]["columns"][1]["required"], true);
    assert!(!schema.to_string().contains("private-password-hash"));
    assert_eq!(fs::read(&path)?, before);
    assert!(schema::inspect(&project, Path::new("missing.sqlite")).await.is_err());
    assert!(!directory.path().join("missing.sqlite").exists());
    assert!(schema::inspect(&project, &path).await.is_err());
    Ok(())
}

#[test]
fn production_environment_refuses_the_mcp_server_without_protocol_or_secret_output() -> Result<()> {
    let directory = common::application()?;
    fs::write(directory.path().join(".env"), "APP_ENV=production\nAPP_KEY=never-print-me")?;
    let output = common::cli(directory.path(), &["mcp"])?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr)?;
    assert!(error.contains("development-only"));
    assert!(!error.contains("never-print-me"));
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn diagnostic_readers_refuse_files_linked_outside_the_project() -> Result<()> {
    use std::os::unix::fs::symlink;
    let directory = common::application()?;
    let outside = tempfile::tempdir()?;
    let secret = outside.path().join("secret");
    fs::write(&secret, "{\"level\":\"ERROR\",\"message\":\"secret\"}\n")?;
    fs::create_dir_all(directory.path().join("storage/logs"))?;
    symlink(&secret, directory.path().join("storage/logs/aurelin.jsonl.2026-11-25"))?;
    symlink(&secret, directory.path().join("external.sqlite"))?;
    let project = Project::open(directory.path())?;
    assert!(logs::recent(&project, 10, false).is_err());
    assert!(schema::inspect(&project, Path::new("external.sqlite")).await.is_err());
    Ok(())
}
