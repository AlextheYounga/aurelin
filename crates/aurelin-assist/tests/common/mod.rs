use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use anyhow::Result;

pub fn application() -> Result<tempfile::TempDir> {
    let directory = tempfile::tempdir()?;
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = 'example-app'\nversion = '0.1.0'\nedition = '2024'\n",
    )?;
    fs::write(
        directory.path().join(".env"),
        "APP_ENV=local\nAPP_KEY=must-not-leak\nDATABASE_URL=must-not-leak-either\n",
    )?;
    Ok(directory)
}

pub fn cli(root: &Path, args: &[&str]) -> Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aurelin-assist"))
        .env_clear()
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .current_dir(root)
        .args(args)
        .output()?)
}
