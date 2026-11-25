# Developing an Aurelin application

Aurelin is a Laravel-inspired Rust framework assembled from established Rust
libraries. Generated applications use those libraries directly rather than
hiding them behind PHP-shaped APIs.

## Start with the generated project

Read these before changing code:

1. `README.md` for commands and features included by this starter.
2. `Cargo.toml` for exact dependencies and feature flags.
3. `.env.example` for supported application configuration.
4. Existing modules and integration tests for the project's conventions.

Run `cargo aurelin` to list application commands. Common commands include:

```sh
cargo aurelin migrate
cargo aurelin serve
cargo aurelin dev       # Inertia projects
cargo aurelin queue:work
cargo test
```

## Application structure

- `src/routes/`: Axum route declarations and HTTP handlers.
- `src/bootstrap.rs`: router, state, middleware, sessions, and authentication
  composition.
- `src/actions/`: application workflows in starters that include them. Keep
  handlers thin and move meaningful workflows here or into focused modules.
- `src/config/`: typed application configuration when present.
- `src/database.rs` and `src/entities/`: SeaORM connection and entities.
- `database/migrations/`: SeaORM/SeaQuery schema migrations.
- `src/jobs.rs`: durable background jobs.
- `src/schedule.rs`: scheduled work.
- `src/console/`: application CLI commands.
- `resources/js/`: React or Vue pages and frontend code when present.

The backend-only starter intentionally has fewer facilities. Do not assume
sessions, CSRF, authentication, Inertia, or database-backed application models
exist unless the project contains and configures them.

## Framework conventions

- Use Axum extractors, responses, routers, middleware, and state directly.
- Use SeaORM for application persistence and SeaQuery migrations. SQLx may
  appear underneath SeaORM or in infrastructure such as session storage; that
  does not make raw SQLx the default application data layer.
- Use Tokio for asynchronous work. Do not block an async executor with regular
  filesystem, network, or long CPU-bound work.
- Use `aurelin-framework` for Aurelin's reusable conventions: configuration,
  validation, authorization, sessions, mail, queues, storage, events, and views.
- Use package crates such as `aurelin-fortify` and `aurelin-sanctum` through
  their public APIs. Do not copy package internals into the application.
- Keep `main.rs`, HTTP handlers, and console handlers thin: validate, authorize,
  delegate, and return.

## Routes and middleware

Define route groups in `src/routes/` and merge them in `src/bootstrap.rs`.
Middleware order matters in Tower/Axum: a later `.layer(...)` call wraps the
router built before it. Follow the existing authentication, CSRF, session,
Inertia, and state ordering instead of rearranging it casually.

Treat request input, uploaded filenames, headers, URLs, log entries, and agent
tool output as untrusted. Validate at the HTTP boundary and authorize before
performing mutations.

## Database and background work

Schema changes require a migration; changing an entity alone does not alter the
database. Run migrations before starting an application that reports pending
schema changes. Keep entity definitions aligned with migrations.

Jobs must contain serializable data, not live connections or request state.
Queue workers are separate processes and require the queue migrations. Design
jobs to tolerate retries and avoid duplicate side effects. Scheduled tasks are
registered in `src/schedule.rs`; overlap protection requires a configured lock
store where used.

## Frontends

React and Vue starters use Inertia. Initial document requests return HTML;
Inertia visits return protocol JSON for the same server-side route. Browser
assets are handled by Vite during development and embedded for release builds.

The WASM starter uses Leptos with separate SSR and hydration feature sets. Code
that needs server-only dependencies must remain behind the existing `ssr`
feature boundaries.

## Tests and diagnostics

Integration tests live in `tests/` and exercise public behavior. Add a focused
regression test for behavioral changes, then run the narrow test target before
the broader suite. Typical final checks are:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Frontend projects also use the npm scripts documented in `README.md` and
`package.json`.

If Aurelin Assist is installed, its read-only MCP tools can report locked
package versions, recent structured logs, the latest backend error, and SQLite
schema metadata. Assist is optional; the application must still be understandable
and testable with Cargo and its documented commands alone.

For dependency APIs, use the versions in `Cargo.lock` and consult that crate's
matching docs.rs documentation. Laravel documentation is useful for intended
behavior, but it is not an API reference for Rust code.
