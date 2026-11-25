use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const EXCLUDED_DIRECTORIES: &[&str] = &[".git", "node_modules", "target", "dist"];
const REQUIRED_TEMPLATE_FILES: &[&str] = &[
    "authentication/.env.example",
    "base/.agents/README.md",
    "base/.agents/docs/aurelin.md",
    "base/.agents/skills/aurelin-development/SKILL.md",
    "base/.agents/skills/aurelin-data-and-jobs/SKILL.md",
    "base/.agents/skills/aurelin-testing-and-debugging/SKILL.md",
    "base/.cargo/config.toml",
    "base/.env.example",
    "base/.gitignore",
    "frontend/.gitignore",
];

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "CARGO_MANIFEST_DIR is not set"))?,
    );
    let output = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "OUT_DIR is not set"))?,
    )
    .join("starters");

    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    for starter in ["base", "authentication", "api", "frontend", "react", "vue", "wasm"] {
        let source = manifest.join("starters").join(starter);
        println!("cargo:rerun-if-changed={}", source.display());
        copy_template(&source, &output.join(starter))?;
    }
    for required in REQUIRED_TEMPLATE_FILES {
        let path = output.join(required);
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("required starter template was not packaged: {}", path.display()),
            ));
        }
    }
    Ok(())
}

fn copy_template(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        if source.file_name().is_some_and(|parent| parent == "bootstrap") && name == "ssr" {
            continue;
        }
        if file_type.is_dir() && EXCLUDED_DIRECTORIES.iter().any(|excluded| name == *excluded) {
            continue;
        }
        let destination = destination.join(&name);
        if file_type.is_dir() {
            copy_template(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
