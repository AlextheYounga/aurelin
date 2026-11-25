//! Laravel-inspired application conventions built on existing Rust libraries.
//!
//! HTTP applications use [`axum`] directly; Aurelin supplies the configuration and
//! starter wiring rather than a second router or handler API.

pub mod auth;
pub mod authorization;
pub mod cache;
pub mod configuration;
pub mod database;
pub mod events;
pub mod http;
pub mod mail;
pub mod notifications;
pub mod queue;
pub mod scheduling;
pub mod session;
pub mod signed_url;
pub mod storage;
pub mod timebox;
pub mod uploads;
pub mod validation;
pub mod view;

pub use axum;
pub use axum_typed_multipart;
pub use inertia_rs as inertia;
