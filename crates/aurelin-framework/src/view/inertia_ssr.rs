//! HTTP integration with the official Inertia server renderer.

use std::{collections::HashMap, error::Error, time::Duration};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{Method, StatusCode, header::CONTENT_LENGTH},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use inertia_rs::HtmlResponseContext;
use reqwest::{header::CONTENT_TYPE, redirect::Policy};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

/// Explicit configuration for an independently managed renderer process.
#[derive(Clone, Debug)]
pub struct SsrConfig {
    /// Full render endpoint, including `/render` or `/__inertia_ssr`.
    pub url: Url,
    /// Maximum time for the entire renderer request.
    pub timeout: Duration,
}

/// Renderer policy configured at application startup.
#[derive(Clone, Debug)]
pub struct SsrSettings {
    pub enabled: bool,
    pub strict: bool,
    pub renderer: SsrConfig,
}

impl SsrSettings {
    /// Parse the application's environment snapshot, without changing process state.
    pub fn from_environment(environment: &HashMap<String, String>) -> Result<Self, Box<dyn Error>> {
        let value = |key: &str, default: &str| environment.get(key).map_or_else(|| default.to_owned(), Clone::clone);
        Ok(Self {
            enabled: value("INERTIA_SSR_ENABLED", "false").parse()?,
            strict: value("INERTIA_SSR_STRICT", "false").parse()?,
            renderer: SsrConfig {
                url: value("INERTIA_SSR_URL", "http://127.0.0.1:13714/render").parse()?,
                timeout: Duration::from_millis(value("INERTIA_SSR_TIMEOUT_MS", "2000").parse()?),
            },
        })
    }
}

/// HTML fragments returned by the Inertia renderer.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct SsrResponse {
    pub head: Vec<String>,
    pub body: String,
}

/// Failures at the renderer boundary.
#[derive(Debug, Error)]
pub enum SsrError {
    #[error("invalid Inertia page data: {0}")]
    Page(#[source] serde_json::Error),
    #[error("SSR request failed: {0}")]
    Request(#[source] reqwest::Error),
    #[error("SSR renderer returned HTTP {0}")]
    Status(reqwest::StatusCode),
    #[error("invalid SSR renderer response: {0}")]
    Response(#[source] serde_json::Error),
}

/// Async renderer client. It never forwards browser request headers or cookies.
#[derive(Clone, Debug)]
pub struct SsrClient {
    config: SsrConfig,
    client: reqwest::Client,
}

impl SsrClient {
    pub fn new(config: SsrConfig) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().redirect(Policy::none()).timeout(config.timeout).build()?;
        Ok(Self { config, client })
    }

    /// Render exactly the page object created by `inertia_rs` for an HTML visit.
    pub async fn render(&self, context: &HtmlResponseContext) -> Result<SsrResponse, SsrError> {
        // Parsing the script-safe JSON preserves the page while validating it before
        // sending it as an ordinary JSON request body.
        let page: serde_json::Value = serde_json::from_str(context.data_page()).map_err(SsrError::Page)?;
        let body = serde_json::to_vec(&page).map_err(SsrError::Page)?;
        let response = self
            .client
            .post(self.config.url.clone())
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(SsrError::Request)?;
        if !response.status().is_success() {
            return Err(SsrError::Status(response.status()));
        }
        let bytes = response.bytes().await.map_err(SsrError::Request)?;
        serde_json::from_slice(&bytes).map_err(SsrError::Response)
    }
}

/// A document function receives the script-safe page and optional trusted SSR fragments.
pub type Document = fn(&HtmlResponseContext, Option<&SsrResponse>) -> String;

#[derive(Clone)]
struct PendingDocument {
    context: HtmlResponseContext,
    document: Document,
}

/// Produce the fallback document and mark it for the async SSR middleware.
///
/// `inertia_rs` calls its HTML callback synchronously and only for HTML visits.
/// Keeping the context in response extensions lets middleware await the renderer
/// without duplicating request classification or page serialization.
pub fn document_response(context: HtmlResponseContext, document: Document) -> Response {
    let mut response = Html(document(&context, None)).into_response();
    response.extensions_mut().insert(PendingDocument { context, document });
    response
}

/// Renderer and failure policy shared by all routes in an application.
#[derive(Clone, Debug, Default)]
pub struct Ssr {
    client: Option<SsrClient>,
    strict: bool,
}

impl Ssr {
    pub fn new(settings: SsrSettings) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: if settings.enabled { Some(SsrClient::new(settings.renderer)?) } else { None },
            strict: settings.strict,
        })
    }
}

/// Resolve only marked GET HTML responses; all other responses pass through unchanged.
pub async fn render_document(State(ssr): State<Ssr>, request: Request, next: Next) -> Response {
    let is_get = request.method() == Method::GET;
    let mut response = next.run(request).await;
    let pending = response.extensions_mut().remove::<PendingDocument>();
    let (Some(client), Some(pending)) = (&ssr.client, pending) else {
        return response;
    };
    if !is_get || !response.status().is_success() {
        return response;
    }
    match client.render(&pending.context).await {
        Ok(rendered) => {
            *response.body_mut() = Body::from((pending.document)(&pending.context, Some(&rendered)));
            response.headers_mut().remove(CONTENT_LENGTH);
            response
        }
        Err(error) => {
            tracing::warn!(%error, "Inertia SSR failed");
            if ssr.strict {
                (StatusCode::INTERNAL_SERVER_ERROR, "Inertia SSR failed").into_response()
            } else {
                response
            }
        }
    }
}
