#[cfg(not(debug_assertions))]
use std::hash::{DefaultHasher, Hash, Hasher};

use kiln_framework::{
    axum::{Router, response::Html},
    inertia::axum::HtmlResponseContext,
};
use vite_rs_axum_0_8::ViteServe;

#[cfg(debug_assertions)]
use crate::frontend_config::{DEV_SCRIPT, FRAMEWORK};

#[cfg(not(debug_assertions))]
const PAGE_MARKER: &str = "__KILN_INERTIA_PAGE__";

#[derive(vite_rs::Embed)]
#[root = "."]
#[output = "./dist"]
#[dev_server_port = "5173"]
pub struct Assets;

#[derive(Clone, Debug, Default)]
pub struct Frontend;

impl Frontend {
    pub fn new() -> Self {
        Self
    }

    pub fn version(&self) -> String {
        asset_version()
    }

    pub fn asset_routes(&self) -> Router<Self> {
        Router::new()
            .nest_service("/build", ViteServe::new(Assets::boxed()))
            .route_service("/@vite/{*path}", ViteServe::new(Assets::boxed()))
            .route_service("/@react-refresh", ViteServe::new(Assets::boxed()))
            .route_service("/@id/{*path}", ViteServe::new(Assets::boxed()))
            .route_service("/@fs/{*path}", ViteServe::new(Assets::boxed()))
            .route_service("/node_modules/{*path}", ViteServe::new(Assets::boxed()))
            .route_service("/resources/{*path}", ViteServe::new(Assets::boxed()))
    }

    pub fn document(&self, context: HtmlResponseContext) -> Html<String> {
        Html(document_html(context.data_page()))
    }

    #[cfg(debug_assertions)]
    pub fn start_dev_server(&self) -> Option<vite_rs::ViteProcess> {
        Assets::start_dev_server()
    }

    #[cfg(not(debug_assertions))]
    pub fn start_dev_server(&self) {}
}

#[cfg(debug_assertions)]
fn document_html(page: &str) -> String {
    let react_refresh = if FRAMEWORK == "react" {
        r#"<script type="module">import RefreshRuntime from "/@react-refresh";RefreshRuntime.injectIntoGlobalHook(window);window.$RefreshReg$=()=>{};window.$RefreshSig$=()=>type=>type;window.__vite_plugin_react_preamble_installed__=true;</script>"#
    } else {
        ""
    };
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Kiln</title>{react_refresh}<script type="module" src="/@vite/client"></script><script type="module" src="/{DEV_SCRIPT}"></script></head><body><script type="application/json" data-page="app">{page}</script><div id="app"></div></body></html>"#,
    )
}

#[cfg(not(debug_assertions))]
fn document_html(page: &str) -> String {
    Assets::get("index.html").map_or_else(
        || "Frontend assets are missing. Build the application in release mode again.".into(),
        |index| String::from_utf8_lossy(&index.bytes).replace(PAGE_MARKER, page),
    )
}

#[cfg(debug_assertions)]
fn asset_version() -> String {
    "development".into()
}

#[cfg(not(debug_assertions))]
fn asset_version() -> String {
    let mut hasher = DefaultHasher::new();
    Assets::get("index.html")
        .map(|index| index.content_hash)
        .hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
