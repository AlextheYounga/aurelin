use std::{collections::HashMap, error::Error, fs};

use aurelin_framework::telemetry;
use serde_json::Value;
use tracing::subscriber::with_default;

#[test]
fn local_file_capture_emits_structured_errors_readable_by_other_processes() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let environment = HashMap::from([("APP_ENV".into(), "local".into()), ("ASSIST_LOG".into(), "true".into())]);
    let subscriber = telemetry::subscriber(directory.path(), &environment)?;
    with_default(subscriber, || {
        tracing::error!(request_id = "request-42", "request failed");
    });
    let files = fs::read_dir(directory.path().join("storage/logs"))?.collect::<Result<Vec<_>, _>>()?;
    let contents = fs::read_to_string(files[0].path())?;
    let entry: Value = serde_json::from_str(contents.trim())?;
    assert_eq!(entry["level"], "ERROR");
    assert_eq!(entry["fields"]["request_id"], "request-42");
    assert_eq!(entry["fields"]["message"], "request failed");
    Ok(())
}

#[test]
fn file_capture_is_disabled_by_default_and_rejected_outside_local_environments() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let subscriber = telemetry::subscriber(directory.path(), &HashMap::new())?;
    with_default(subscriber, || tracing::info!("terminal only"));
    assert!(!directory.path().join("storage").exists());
    let production = HashMap::from([("APP_ENV".into(), "production".into()), ("ASSIST_LOG".into(), "true".into())]);
    assert!(matches!(telemetry::subscriber(directory.path(), &production), Err(telemetry::TelemetryError::NonLocal)));
    assert!(!directory.path().join("storage").exists());
    Ok(())
}

#[test]
fn invalid_logging_filters_fail_instead_of_silently_disabling_diagnostics() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let environment = HashMap::from([("RUST_LOG".into(), "[invalid".into())]);
    assert!(telemetry::subscriber(directory.path(), &environment).is_err());
    Ok(())
}

#[cfg(not(all(feature = "tokio-console", tokio_unstable)))]
#[test]
fn requesting_console_without_instrumentation_gives_an_actionable_error() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let environment = HashMap::from([("TOKIO_CONSOLE".into(), "true".into())]);
    assert!(matches!(
        telemetry::subscriber(directory.path(), &environment),
        Err(telemetry::TelemetryError::ConsoleUnavailable)
    ));
    Ok(())
}
