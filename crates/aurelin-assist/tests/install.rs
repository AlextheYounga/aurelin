mod common;

use std::fs;

use anyhow::Result;
use serde_json::{Value, json};

#[test]
fn install_and_update_preserve_user_instructions_and_custom_mcp_servers() -> Result<()> {
    let directory = common::application()?;
    let root = directory.path();
    fs::write(root.join("AGENTS.md"), "Keep our business rules.\n")?;
    fs::write(
        root.join("opencode.json"),
        json!({
            "model": "custom/model",
            "mcp": {"servers": {"existing": {"type": "local", "command": ["custom-tool"]}}}
        })
        .to_string(),
    )?;
    let installed = common::cli(root, &["install", "--playwright"])?;
    assert!(installed.status.success(), "{}", String::from_utf8_lossy(&installed.stderr));
    let agents = fs::read_to_string(root.join("AGENTS.md"))?;
    assert!(agents.starts_with("Keep our business rules.\n"));
    fs::write(
        root.join("AGENTS.md"),
        format!("{}\nKeep this new decision too.\n", agents.replace("## Aurelin Assist", "## Outdated advice")),
    )?;
    let skill_path = root.join(".agents/skills/aurelin-debugging/SKILL.md");
    let skill = fs::read_to_string(&skill_path)?;
    fs::write(&skill_path, skill.replace("# Debugging Aurelin applications", "# Outdated debugging"))?;
    let config: Value = serde_json::from_str(&fs::read_to_string(root.join("opencode.json"))?)?;
    assert_eq!(config["model"], "custom/model");
    assert_eq!(config["mcp"]["servers"]["existing"]["command"], json!(["custom-tool"]));
    assert_eq!(config["mcp"]["servers"]["aurelin-assist"]["command"][0], "aurelin-assist");
    assert!(
        config["mcp"]["servers"]["playwright"]["command"]
            .as_array()
            .is_some_and(|args| args.contains(&json!("--isolated")))
    );
    let updated = common::cli(root, &["update"])?;
    assert!(updated.status.success(), "{}", String::from_utf8_lossy(&updated.stderr));
    let updated_agents = fs::read_to_string(root.join("AGENTS.md"))?;
    assert_eq!(updated_agents, format!("{agents}\nKeep this new decision too.\n"));
    assert_eq!(fs::read_to_string(skill_path)?, skill);
    let updated_config: Value = serde_json::from_str(&fs::read_to_string(root.join("opencode.json"))?)?;
    assert_eq!(updated_config, config);
    let generic: Value = serde_json::from_str(&fs::read_to_string(root.join(".mcp.json"))?)?;
    assert_eq!(generic["mcpServers"]["aurelin-assist"]["command"], "aurelin-assist");
    assert!(!fs::read_to_string(root.join("opencode.json"))?.contains("must-not-leak"));
    Ok(())
}

#[test]
fn malformed_configuration_and_ambiguous_blocks_fail_before_any_resource_is_written() -> Result<()> {
    let directory = common::application()?;
    let root = directory.path();
    let original = "User rules\n";
    fs::write(root.join("AGENTS.md"), original)?;
    fs::write(root.join("opencode.json"), "{ broken json")?;
    assert!(!common::cli(root, &["install"])?.status.success());
    assert_eq!(fs::read_to_string(root.join("AGENTS.md"))?, original);
    assert!(!root.join("CLAUDE.md").exists());
    fs::write(root.join("opencode.json"), "{}")?;
    let ambiguous = "<!-- aurelin-assist:start -->\nMissing end marker";
    fs::write(root.join("AGENTS.md"), ambiguous)?;
    assert!(!common::cli(root, &["install"])?.status.success());
    assert_eq!(fs::read_to_string(root.join("AGENTS.md"))?, ambiguous);
    assert!(!root.join("CLAUDE.md").exists());
    Ok(())
}

#[test]
fn existing_jsonc_configuration_is_not_shadowed_or_modified() -> Result<()> {
    let directory = common::application()?;
    let root = directory.path();
    let config = "{ /* user comments */ }";
    fs::write(root.join("opencode.jsonc"), config)?;
    assert!(!common::cli(root, &["install"])?.status.success());
    assert_eq!(fs::read_to_string(root.join("opencode.jsonc"))?, config);
    assert!(!root.join("opencode.json").exists());
    assert!(!root.join("AGENTS.md").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn installer_refuses_symlinked_instruction_files_and_resource_directories() -> Result<()> {
    use std::os::unix::fs::symlink;
    let directory = common::application()?;
    let outside = tempfile::tempdir()?;
    let target = outside.path().join("AGENTS.md");
    fs::write(&target, "Other project")?;
    symlink(&target, directory.path().join("AGENTS.md"))?;
    assert!(!common::cli(directory.path(), &["install"])?.status.success());
    assert_eq!(fs::read_to_string(target)?, "Other project");
    let second = common::application()?;
    symlink(outside.path(), second.path().join(".agents"))?;
    assert!(!common::cli(second.path(), &["install"])?.status.success());
    assert!(!outside.path().join("skills").exists());
    Ok(())
}
