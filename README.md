# Aurelin

Aurelin is an early Rust framework ecosystem modeled on Laravel 13. It combines
existing Rust crates and frontend packages into a recognizable application
workflow rather than reimplementing their underlying functionality.

## Current state

The repository currently provides foundational, tested behavior:

- creation of a Cargo application from an embedded starter;
- `.env` and typed listener configuration;
- Axum routes and Tokio HTTP serving;
- a health endpoint and graceful shutdown;
- an Inertia 3 Axum integration verified at the protocol boundary;
- framework-owned authentication primitives and a separate headless
  `aurelin-fortify` package;
- minimal React 19 and Vue 3 Inertia starters with Vite development and
  embedded production assets;
- a shared SQLite authentication foundation with registration, login, logout,
  remember-me sessions, login throttling, CSRF protection, persisted sessions,
  password reset, email verification, password confirmation, two-factor
  authentication, passkeys, account settings, and SMTP mail delivery.

The authentication starter now covers the principal built-in Laravel 13 React
and Vue flows, but it is not yet a complete parity claim. See `CONTEXT.md` for
the target and `docs/ARCHITECTURE.md` for exact implementation status and
intentional scope exclusions.

## Try the backend foundation

Until the crates are published, pass the local framework path explicitly:

```sh
cargo run -p aurelin-cli -- new /tmp/my-aurelin-app \
  --framework-path "$PWD/crates/aurelin-framework"
cd /tmp/my-aurelin-app
cargo run
```

Then open <http://127.0.0.1:8000>. This command creates a backend-only starter;
it does not imply that a finished Laravel-equivalent starter kit exists.

To exercise one of the frontend foundations:

```sh
cargo run -p aurelin-cli -- new /tmp/my-aurelin-react-app \
  --framework-path "$PWD/crates/aurelin-framework" \
  --frontend react
cd /tmp/my-aurelin-react-app
npm install
cargo run
```

Use `--frontend vue` for Vue. Both variants provide the same backend behavior
and authentication workflow.

Generate shell completion output with, for example:

```sh
cargo run -p aurelin-cli -- completions bash
```

## Develop Aurelin

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
