use std::{fs, path::Path};

use anyhow::{Result, bail};

pub(super) fn install_api_registration(path: &Path) -> Result<()> {
    fn replace(path: &Path, from: &str, to: &str) -> Result<()> {
        let contents = fs::read_to_string(path)?;
        if !contents.contains(from) {
            bail!("API registration anchor missing in {}", path.display());
        }
        fs::write(path, contents.replacen(from, to, 1))?;
        Ok(())
    }
    replace(&path.join("src/lib.rs"), "mod actions;", "mod actions;\nmod api;")?;
    let migrations = path.join("database/migrations/mod.rs");
    replace(
        &migrations,
        "mod m20261125_000001_create_posts;",
        "mod m20261125_000001_create_posts;\nmod m20261125_000001_create_personal_access_tokens;",
    )?;
    replace(
        &migrations,
        "Box::new(m20261125_000001_create_posts::Migration),",
        "Box::new(m20261125_000001_create_posts::Migration),\n            Box::new(m20261125_000001_create_personal_access_tokens::Migration),",
    )?;
    let bootstrap = path.join("src/bootstrap.rs");
    replace(
        &bootstrap,
        "    let session_store =",
        "    let token_service =\n        aurelin_sanctum::TokenService::new(crate::api::Repository(database.clone()));\n    let bearer_routes = crate::api::bearer_routes(token_service.clone(), database.clone());\n    let session_store =",
    )?;
    replace(
        &bootstrap,
        "        .merge(crate::routes::http::routes())",
        "        .merge(crate::api::browser_routes(token_service))\n        .merge(crate::routes::http::routes())",
    )?;
    replace(
        &bootstrap,
        "        .with_state(frontend))",
        "        .with_state(frontend)\n        .merge(bearer_routes))",
    )?;
    Ok(())
}
