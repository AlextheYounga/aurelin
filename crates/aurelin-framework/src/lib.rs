//! Laravel-inspired application conventions built on existing Rust libraries.
//!
//! HTTP applications use [`axum`] directly; Aurelin supplies the configuration and
//! starter wiring rather than a second router or handler API.

pub mod auth;
pub mod authorization;
pub mod configuration;
pub mod database;
pub mod http;
pub mod mail;
pub mod queue;
pub mod session;
pub mod signed_url;
pub mod timebox;
pub mod view;

pub use axum;
pub use inertia_rs as inertia;
