//! HTTP integration with the official Inertia server renderer.

use std::time::Duration;

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
