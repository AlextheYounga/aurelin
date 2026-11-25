//! Managed resource updates. Files are preflighted before writing and replaced
//! atomically one at a time; failures retain all existing user-owned content.

use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::project::Project;

const START: &str = "<!-- aurelin-assist:start -->";
const END: &str = "<!-- aurelin-assist:end -->";
const SKILLS: &[&str] = &[".agents/skills/aurelin-debugging/SKILL.md", ".claude/skills/aurelin-debugging/SKILL.md"];
const PLAYWRIGHT: &str = "@playwright/mcp@0.0.86";

pub fn install(project: &Project, playwright: bool) -> Result<()> {
    // JSONC must not be silently shadowed or rewritten as JSON. The installer
    // deliberately supports strict JSON only; manual integration is documented.
    for path in ["opencode.jsonc", ".opencode/opencode.jsonc", ".opencode/opencode.json"] {
        if project.root().join(path).try_exists()? {
            bail!(
                "{path} already exists; register Assist manually there (see docs/AI_ASSIST.md), rather than introducing conflicting OpenCode configuration"
            );
        }
    }
    let mut files = Vec::new();
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = writable_path(project, name)?;
        let old = read_optional(&path)?;
        let block = format!("{START}\n{}\n{END}", include_str!("../resources/guidelines.md").trim());
        files.push((path, merge(&old, &block)?));
    }
    let skill = include_str!("../resources/SKILL.md");
    for name in SKILLS {
        let path = writable_path(project, name)?;
        let old = read_optional(&path)?;
        if !old.is_empty() && !old.contains(START) {
            bail!("{name} is user-owned; rename it before installing the managed Aurelin skill");
        }
        let contents = if old.is_empty() {
            skill.to_owned()
        } else {
            let start = skill.find(START).context("embedded skill has no managed block")?;
            merge(&old, &skill[start..])?
        };
        files.push((path, contents));
    }
    for (name, opencode) in [(".mcp.json", false), ("opencode.json", true)] {
        let path = writable_path(project, name)?;
        let old = read_optional(&path)?;
        let mut config: Value = if old.is_empty() {
            json!({})
        } else {
            serde_json::from_str(&old).with_context(|| format!("{name} must be valid JSON; file left unchanged"))?
        };
        configure(&mut config, project.root(), playwright, opencode)?;
        files.push((path, format!("{}\n", serde_json::to_string_pretty(&config)?)));
    }
    for (path, contents) in files {
        let parent = path.parent().context("resource has no parent directory")?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        if let Ok(metadata) = fs::metadata(&path) {
            file.as_file().set_permissions(metadata.permissions())?;
        }
        file.write_all(contents.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(&path).with_context(|| {
            format!("could not replace {}; earlier resources may have been updated", path.display())
        })?;
    }
    Ok(())
}

pub fn update(project: &Project) -> Result<()> {
    let agents = read_optional(&writable_path(project, "AGENTS.md")?)?;
    if !agents.contains(START) {
        bail!("Assist is not installed; run `aurelin-assist install` first");
    }
    // Existing companion entries, including user customization, are preserved.
    install(project, false)
}

fn configure(config: &mut Value, root: &Path, playwright: bool, opencode: bool) -> Result<()> {
    let object = config.as_object_mut().context("MCP configuration must be a JSON object")?;
    let servers = if opencode {
        object.entry("$schema").or_insert(json!("https://opencode.ai/config.json"));
        let mcp = object.entry("mcp").or_insert(json!({})).as_object_mut().context("mcp must be an object")?;
        // Don't mix V1 server maps with V2's mcp.servers.
        if mcp.values().any(|value| value.get("type").is_some()) {
            bail!("existing OpenCode MCP configuration uses V1 syntax; migrate it or register Assist manually");
        }
        mcp.entry("servers").or_insert(json!({})).as_object_mut().context("mcp.servers must be an object")?
    } else {
        object.entry("mcpServers").or_insert(json!({})).as_object_mut().context("mcpServers must be an object")?
    };
    servers.entry("aurelin-assist").or_insert_with(|| {
        if opencode {
            json!({"type": "local", "command": ["aurelin-assist", "--project", root, "mcp"], "cwd": root})
        } else {
            json!({"command": "aurelin-assist", "args": ["--project", root, "mcp"]})
        }
    });
    if playwright {
        servers.entry("playwright").or_insert_with(|| {
            if opencode {
                json!({"type": "local", "command": ["npx", "-y", PLAYWRIGHT, "--isolated", "--headless"], "cwd": root})
            } else {
                json!({"command": "npx", "args": ["-y", PLAYWRIGHT, "--isolated", "--headless"]})
            }
        });
    }
    Ok(())
}

fn read_optional(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn merge(old: &str, block: &str) -> Result<String> {
    if !old.contains(START) && !old.contains(END) {
        let separator = if old.is_empty() || old.ends_with('\n') { "" } else { "\n" };
        return Ok(format!("{old}{separator}{block}\n"));
    }
    if old.matches(START).count() != 1 || old.matches(END).count() != 1 {
        bail!("managed resource contains ambiguous markers; repair the aurelin-assist block before updating");
    }
    let start = old.find(START).context("missing start marker")?;
    let end = old.find(END).context("missing end marker")?;
    if end < start {
        bail!("managed resource has reversed markers; file left unchanged");
    }
    Ok(format!("{}{}{}", &old[..start], block.trim_end(), &old[end + END.len()..]))
}

fn writable_path(project: &Project, relative: &str) -> Result<PathBuf> {
    let path = project.root().join(relative);
    // Reject symlinked files and parents, even when dangling. This prevents an
    // installer from writing into another project through .agents or a file link.
    let mut current = path.as_path();
    while current != project.root() {
        match fs::symlink_metadata(current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("refusing to update symlink {}", current.display())
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        current = current.parent().context("resource escaped the project")?;
    }
    Ok(path)
}
