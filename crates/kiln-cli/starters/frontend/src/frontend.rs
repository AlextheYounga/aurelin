use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io,
    path::PathBuf,
    sync::Arc,
};

#[cfg(debug_assertions)]
use axum_vite::spawn_dev_server;
use axum_vite::{
    DevServerHandle, EntryAssets, ViteConfig, frameworks::Framework, router as asset_router,
};
use kiln_framework::{
    axum::{Router, response::Html},
    inertia::axum::HtmlResponseContext,
};

use crate::frontend_config::{DEV_SCRIPT, FRAMEWORK, MANIFEST_KEY};

#[derive(Clone, Debug)]
pub struct Frontend {
    config: Arc<ViteConfig>,
    entry: EntryAssets,
    version: String,
}

impl Default for Frontend {
    fn default() -> Self {
        Self::new()
    }
}

impl Frontend {
    pub fn new() -> Self {
        let framework = match FRAMEWORK {
            "react" => Framework::React,
            "vue" => Framework::Vue,
            _ => Framework::None,
        };
        let config = ViteConfig {
            prefix: "/build/".into(),
            frontend_root: Some(PathBuf::from(".")),
            framework,
            dev_script: DEV_SCRIPT.into(),
            manifest_key: MANIFEST_KEY.into(),
            ..ViteConfig::from_env(axum_vite::embedded_dir!("$CARGO_MANIFEST_DIR/dist"))
        };
        let entry = config.entry_assets();
        let version = asset_version(&config);
        Self {
            config: Arc::new(config),
            entry,
            version,
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn asset_routes(&self) -> Router<Self> {
        asset_router((*self.config).clone())
    }

    pub fn document(&self, context: HtmlResponseContext) -> Html<String> {
        let stylesheets = self
            .entry
            .stylesheets
            .iter()
            .map(|href| format!(r#"<link rel="stylesheet" href="{href}">"#))
            .collect::<String>();
        Html(format!(
            r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Kiln</title>{hmr}{stylesheets}<script type="module" src="{script}"></script></head><body><script type="application/json" data-page="app">{page}</script><div id="app"></div></body></html>"#,
            hmr = self.config.hmr_scripts(),
            script = self.entry.script,
            page = context.data_page(),
        ))
    }

    #[cfg(debug_assertions)]
    pub fn start_dev_server(&self) -> io::Result<Option<DevServerHandle>> {
        spawn_dev_server(&self.config).map(Some)
    }

    #[cfg(not(debug_assertions))]
    pub fn start_dev_server(&self) -> io::Result<Option<DevServerHandle>> {
        Ok(None)
    }
}

fn asset_version(config: &ViteConfig) -> String {
    let mut hasher = DefaultHasher::new();
    if let Some(manifest) = config
        .dir
        .and_then(|dir| dir.get_file(".vite/manifest.json"))
    {
        manifest.contents().hash(&mut hasher);
    } else {
        "development".hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}
