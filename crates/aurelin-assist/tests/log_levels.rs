use std::fs;

use anyhow::Result;
use aurelin_assist::{logs, project::Project};

#[test]
fn last_error_matches_structured_severity_not_error_text_inside_later_messages() -> Result<()> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join("Cargo.toml"), "[package]\nname = 'logs-fixture'\nversion = '0.1.0'\n")?;
    let root = directory.path().join("storage/logs");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("aurelin.jsonl.2026-11-25"),
        concat!(
            "{\"level\":\"ERROR\",\"message\":\"real failure\\nstack frame\"}\n",
            "{\"level\":\"INFO\",\"message\":\"replayed ERROR: unrelated\"}\n"
        ),
    )?;
    let errors = logs::recent(&Project::open(directory.path())?, 1, true)?;
    assert_eq!(errors[0]["message"], "real failure\nstack frame");
    Ok(())
}
