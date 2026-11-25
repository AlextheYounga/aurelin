---
name: aurelin-development
description: Build or change Aurelin routes, handlers, middleware, configuration, authentication flows, frontend pages, or application structure.
---

# Aurelin application development

Read `../../docs/aurelin.md`, the project `README.md`, `Cargo.toml`, and the
nearby source before editing.

1. Identify whether the capability belongs to this application or to an
   Aurelin/framework package. Keep application-specific routes, workflows,
   configuration, and views here; do not copy reusable package behavior into
   the project.
2. Follow the generated structure. Declare routes in `src/routes/`, compose
   state and middleware in `src/bootstrap.rs`, and put meaningful workflows in
   `src/actions/` or another focused application module.
3. Use Axum and the selected ecosystem crates directly. Check locked dependency
   versions before choosing APIs; Laravel docs describe behavior, not Rust APIs.
4. Validate untrusted input and authorize before mutations. Preserve existing
   session, CSRF, authentication, Inertia, and middleware ordering.
5. Add a focused integration test for changed behavior and run the relevant
   Cargo and frontend checks documented by the project.
