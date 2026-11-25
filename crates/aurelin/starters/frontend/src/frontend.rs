#[cfg(not(debug_assertions))]
use std::hash::{DefaultHasher, Hash, Hasher};

use aurelin_framework::{
    axum::{Router, response::Response},
    inertia::axum::HtmlResponseContext,
    view::inertia_ssr::{Ssr, SsrResponse, document_response},
};
use vite_rs_axum_0_8::ViteServe;

use crate::config::frontend::{
    CONFIRM_PASSWORD_COMPONENT, DASHBOARD_COMPONENT, FORGOT_PASSWORD_COMPONENT, LOGIN_COMPONENT,
    REGISTER_COMPONENT, RESET_PASSWORD_COMPONENT, TWO_FACTOR_CHALLENGE_COMPONENT,
    VERIFY_EMAIL_COMPONENT,
};

#[cfg(debug_assertions)]
use crate::config::frontend::{DEV_SCRIPT, FRAMEWORK};

#[cfg(not(debug_assertions))]
const BODY_MARKER: &str = "__AURELIN_INERTIA_BODY__";

#[derive(vite_rs::Embed)]
#[root = "."]
#[output = "./dist"]
#[dev_server_port = "5173"]
pub struct Assets;

#[derive(Clone, Debug, Default)]
pub struct Frontend {
    ssr: Ssr,
}

impl Frontend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ssr(mut self, ssr: Ssr) -> Self {
        self.ssr = ssr;
        self
    }

    pub fn ssr(&self) -> Ssr {
        self.ssr.clone()
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

    pub fn document(&self, context: HtmlResponseContext) -> Response {
        document_response(context, document_html)
    }

    #[cfg(debug_assertions)]
    pub fn start_dev_server(&self) -> Option<vite_rs::ViteProcess> {
        Assets::start_dev_server()
    }

    #[cfg(not(debug_assertions))]
    pub fn start_dev_server(&self) {}
}

impl aurelin_fortify::axum::FortifyFrontend for Frontend {
    fn fortify_components(&self) -> aurelin_fortify::axum::FortifyComponents {
        aurelin_fortify::axum::FortifyComponents {
            dashboard: DASHBOARD_COMPONENT,
            login: LOGIN_COMPONENT,
            register: REGISTER_COMPONENT,
            forgot_password: FORGOT_PASSWORD_COMPONENT,
            reset_password: RESET_PASSWORD_COMPONENT,
            confirm_password: CONFIRM_PASSWORD_COMPONENT,
            verify_email: VERIFY_EMAIL_COMPONENT,
            two_factor_challenge: TWO_FACTOR_CHALLENGE_COMPONENT,
        }
    }

    fn fortify_document(&self, context: HtmlResponseContext) -> Response {
        self.document(context)
    }
}

#[cfg(debug_assertions)]
fn document_html(context: &HtmlResponseContext, rendered: Option<&SsrResponse>) -> String {
    let page = context.data_page();
    let head = rendered.map_or_else(String::new, |ssr| ssr.head.join("\n"));
    let title = if head.contains("<title") {
        ""
    } else {
        "<title>Aurelin</title>"
    };
    let body = rendered.map_or_else(
        || format!(r#"<script type="application/json" data-page="app">{page}</script><div id="app"></div>"#),
        |ssr| ssr.body.clone(),
    );
    let react_refresh = if FRAMEWORK == "react" {
        r#"<script type="module">import RefreshRuntime from "/@react-refresh";RefreshRuntime.injectIntoGlobalHook(window);window.$RefreshReg$=()=>{};window.$RefreshSig$=()=>type=>type;window.__vite_plugin_react_preamble_installed__=true;</script>"#
    } else {
        ""
    };
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">{title}{head}{react_refresh}<script type="module" src="/@vite/client"></script><script type="module" src="/{DEV_SCRIPT}"></script></head><body>{body}</body></html>"#,
    )
}

#[cfg(not(debug_assertions))]
fn document_html(context: &HtmlResponseContext, rendered: Option<&SsrResponse>) -> String {
    Assets::get("index.html").map_or_else(
        || "Frontend assets are missing. Build the application in release mode again.".into(),
        |index| {
            let html = String::from_utf8_lossy(&index.bytes);
            if let Some(ssr) = rendered {
                let head = ssr.head.join("\n");
                let html = if head.contains("<title") { html.replace("<title>Aurelin</title>", "") } else { html.into_owned() };
                html.replace("</head>", &format!("{head}</head>")).replace(BODY_MARKER, &ssr.body)
            } else {
                html.replace(
                    BODY_MARKER,
                    &format!(
                        r#"<script type="application/json" data-page="app">{}</script><div id="app"></div>"#,
                        context.data_page()
                    ),
                )
            }
        },
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
