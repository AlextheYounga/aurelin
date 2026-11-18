//! Laravel-inspired application conventions built on existing Rust libraries.
//!
//! HTTP applications use [`axum`] directly; Aurelin supplies the configuration and
//! starter wiring rather than a second router or handler API.

pub mod configuration;

pub use axum;
pub use inertia_rs as inertia;
