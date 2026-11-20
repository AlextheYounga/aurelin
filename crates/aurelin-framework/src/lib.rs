//! Laravel-inspired application conventions built on existing Rust libraries.
//!
//! HTTP applications use [`axum`] directly; Aurelin supplies the configuration and
//! starter wiring rather than a second router or handler API.

pub mod auth;
pub mod configuration;
pub mod http;
pub mod mail;
pub mod queue;
pub mod session;
pub mod signed_url;
pub mod timebox;

pub use axum;
pub use inertia_rs as inertia;
