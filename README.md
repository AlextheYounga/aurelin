# Aurelin

**A productive web application framework for Rust, inspired by Laravel.**

Aurelin brings proven application conventions to Rust without replacing the
ecosystem that already works. Axum handles HTTP. SeaORM handles application
data. Inertia connects the backend to React or Vue. Aurelin makes those parts
work together as one coherent framework.

The goal is simple: start with a complete application, not a collection of
integration decisions.

> [!IMPORTANT]
> Aurelin is under active development. Version 0.1.0 is available on crates.io,
> while this branch contains unreleased guided-generation and queue work. It is
> suitable for exploring and contributing, but is not yet recommended for
> production applications.

## What you get

- Axum routing and Tokio HTTP serving
- Typed environment configuration
- Graceful shutdown and health checks
- Inertia 3 integration for React 19 and Vue 3
- Vite development and embedded production assets
- SQLite persistence with SeaORM
- Persisted sessions and CSRF protection
- Registration, login, logout, and remember-me authentication
- Login throttling and password confirmation
- Password reset and email verification
- Two-factor authentication and recovery codes
- Passkey registration and authentication
- Account and profile settings
- SMTP mail delivery
- SQLite-backed background jobs and workers

Authentication is split deliberately between reusable packages and generated
application code. Aurelin provides the secure workflows and contracts; your
application owns its users, database models, migrations, policies, mail
content, and interface.

## Quick start

Until the next coordinated crate release, clone the repository and generate a
React application against its matching local framework source:

```sh
cargo run -p aurelin-cli -- new /tmp/my-aurelin-app \
  --framework-path "$PWD/crates/aurelin-framework" \
  --frontend react

cd /tmp/my-aurelin-app
npm install
cargo run
```

Open <http://127.0.0.1:8000> and register an account.

Use `--frontend vue` for Vue, or omit `--frontend` to generate a backend-only
application:

```sh
cargo run -p aurelin-cli -- new /tmp/my-aurelin-api \
  --framework-path "$PWD/crates/aurelin-framework"
```

The `--framework-path` option keeps unreleased CLI and framework changes on the
same version. Released CLI versions generate registry dependencies directly.

## The Aurelin approach

Rust has excellent web libraries. Aurelin does not hide or duplicate them. It
chooses a well-matched stack, supplies the missing application conventions,
and gives each concern a clear home.

| Concern | Aurelin choice |
|---|---|
| HTTP and middleware | Axum and Tower |
| Async runtime | Tokio |
| Data and migrations | SeaORM |
| Server-driven frontend | Inertia |
| Sessions | tower-sessions |
| Mail | lettre |
| Passkeys | webauthn-rs |
| Frontend | React or Vue with Vite |

Applications still use these libraries through their native APIs. Aurelin is
the convention and integration layer, not a second abstraction over
everything beneath it.

## Packages

| Package | Responsibility |
|---|---|
| `aurelin-framework` | Core configuration, HTTP, sessions, mail, encryption, signed URLs, rate limiting, and authentication primitives |
| `aurelin-fortify` | Headless authentication workflows and Axum integration |
| `aurelin-passkeys` | WebAuthn registration and authentication ceremonies |
| `aurelin-cli` | Application generation and shell completions |

## Project status

The authentication starter covers the principal Laravel 13 React and Vue
flows, but Aurelin does not yet claim complete Laravel parity.

- [`CONTEXT.md`](CONTEXT.md) defines the target and project constraints.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) tracks completed and planned work.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) explains package ownership and
  intentional scope boundaries.
- [`docs/crate-mapping.md`](docs/crate-mapping.md) records the selected Rust
  ecosystem components.

## Development

Run the complete validation suite from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Generate shell completions with:

```sh
cargo run -p aurelin-cli -- completions bash
```

Replace `bash` with another shell supported by `clap_complete`.
