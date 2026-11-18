# Kiln

Kiln is an early Rust framework ecosystem modeled on Laravel 13. It combines
existing Rust crates and frontend packages into a recognizable application
workflow rather than reimplementing their underlying functionality.

## Current state

The repository currently provides foundational, tested behavior:

- creation of a Cargo application from an embedded starter;
- `.env` and typed listener configuration;
- Axum routes and Tokio HTTP serving;
- a health endpoint and graceful shutdown;
- an Inertia 3 Axum integration verified at the protocol boundary;
- minimal React 19 and Vue 3 Inertia starters with Vite development and
  embedded production assets;
- a shared SQLite authentication foundation with registration, login, logout,
  login throttling, CSRF protection, persisted sessions, and password-reset
  screens and token handling.

The authentication starter is still incomplete: email verification, password
confirmation, production mail delivery, and the remaining Laravel starter-kit
behavior are not implemented. See `CONTEXT.md` for the target and
`docs/ARCHITECTURE.md` for exact implementation status and references.

## Try the backend foundation

Until the crates are published, pass the local framework path explicitly:

```sh
cargo run -p kiln-cli -- new /tmp/my-kiln-app \
  --framework-path "$PWD/crates/kiln-framework"
cd /tmp/my-kiln-app
cargo run
```

Then open <http://127.0.0.1:8000>. This command creates a backend-only starter;
it does not imply that a finished Laravel-equivalent starter kit exists.

To exercise one of the frontend foundations:

```sh
cargo run -p kiln-cli -- new /tmp/my-kiln-react-app \
  --framework-path "$PWD/crates/kiln-framework" \
  --frontend react
cd /tmp/my-kiln-react-app
npm install
cargo run
```

Use `--frontend vue` for Vue. Both variants currently provide the same partial
authentication workflow; they should not yet be described as complete Laravel
authentication starters.

Generate shell completion output with, for example:

```sh
cargo run -p kiln-cli -- completions bash
```

## Develop Kiln

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
