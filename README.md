# Aurelin

**A productive web application framework for Rust, inspired by Laravel.**

Aurelin brings proven application conventions to Rust without replacing the
ecosystem that already works. Axum handles HTTP. SeaORM handles application
data. Inertia connects the backend to React or Vue, while Leptos provides a
native full-stack Rust/WASM path. Aurelin makes those parts work together as
coherent application conventions.

The goal is simple: start with a complete application, not a collection of
integration decisions.

> [!IMPORTANT]
> Aurelin is under active development. It is suitable for exploring and
> contributing, but is not yet recommended for production applications.

## What you get

- Axum routing and Tokio HTTP serving
- Typed environment configuration
- Graceful shutdown and health checks
- Inertia 3 integration for React 19 and Vue 3
- Vite development and embedded production assets
- Leptos SSR and WebAssembly hydration without Node.js
- SQLite persistence with SeaORM
- Persisted sessions and CSRF protection
- Registration, login, logout, and remember-me authentication
- Login throttling and password confirmation
- Password reset and email verification
- Two-factor authentication and recovery codes
- Passkey registration and authentication
- Account and profile settings
- Application-defined authorization gates and policies
- SMTP mail delivery
- SQLite-backed background jobs, retries, failed-job operations, and workers
- Typed process-local and Redis caching with atomic owner-safe locks

Authentication is split deliberately between reusable packages and generated
application code. Aurelin provides the secure workflows and contracts; your
application owns its users, database models, migrations, policies, mail
content, and interface.

## Quick start

Install Aurelin and generate a React application:

```sh
cargo install aurelin
aurelin new /tmp/my-aurelin-app --frontend react

cd /tmp/my-aurelin-app
npm install
cargo aurelin migrate
cargo aurelin dev
```

Open <http://127.0.0.1:8000> and register an account.
Run `cargo aurelin` inside the generated project to list its application
commands.

Register scheduled callbacks in `src/schedule.rs`, then invoke
`cargo aurelin schedule:run` once per minute from your system scheduler. Use a
shared Redis cache for overlap prevention across separate processes. See
`docs/ARCHITECTURE.md` for the supported cron subset and scheduler limitations.

The former `aurelin-cli` package remains on crates.io for existing users, but
new releases use the `aurelin` package name.

Use `--frontend vue` for Vue, `--frontend wasm` for the Rust-native Leptos
foundation, or omit `--frontend` to generate a backend-only application:

```sh
aurelin new /tmp/my-aurelin-api
```

Generated applications use the matching Aurelin crates from crates.io. Aurelin
contributors can pass `--framework-path` to test against a local framework
checkout instead.

### Personal access tokens

React and Vue applications can opt into bearer-token API routes:

```sh
aurelin new my-app --frontend react --api
```

The backend-only and WASM starters do not currently support this option: they
do not share the browser authentication composition used for token management.
Run `cargo aurelin migrate` before serving the generated application.

Browser sessions remain cookie-authenticated and CSRF-protected. A signed-in
user can `POST /tokens` with a CSRF token and JSON such as
`{"name":"laptop","abilities":["read"],"expires_at":null}`; the response
contains the plaintext credential only once. `GET /tokens` lists the owner's
metadata and `DELETE /tokens/{id}` revokes an owned token. Issuance is rate
limited; wildcard (`*`) issuance is denied until the application explicitly
changes its policy. Expiration is an optional Unix timestamp in seconds.

Send `Authorization: Bearer <credential>` to `GET /api/user`, or
`DELETE /api/token` to revoke the current credential. These routes do not accept
session cookies as authentication, and bearer headers do not bypass CSRF on
browser routes. Only a SHA-256 digest is stored; protect the plaintext token as
a password. Abilities constrain token access but do not replace application
authorization policies. This is not OAuth or JWT authentication.

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
| Full-stack Rust/WASM | Leptos and cargo-leptos |
| Sessions | tower-sessions |
| Cache | Moka and Fred/Redis through Aurelin's typed cache API |
| Authorization | Aurelin gates and application-owned policies over axum-login |
| Mail | lettre |
| Passkeys | webauthn-rs |
| Frontend | React or Vue with Vite; Rust/WASM with Leptos |

Applications still use these libraries through their native APIs. Aurelin is
the convention and integration layer, not a second abstraction over
everything beneath it.

For JSON APIs, `aurelin_framework::validation::http::Validated` composes
`validator` and Axum extractors for JSON, forms, query strings, and paths. It
returns safe JSON errors, including structured rule codes, on invalid input.
Apply authorization in an outer route layer or an earlier parts extractor:
authorization inside the handler runs after validation. For browser/Inertia
forms, use `validation::browser::BrowserValidated` instead: implement
`OldInput` on the request type to explicitly select non-sensitive plain-text
fields, install `flash_middleware` inside the session layer, and call
`validation_page_props` with each Inertia page's props before rendering.
Never select passwords, credentials, tokens, secrets, or uploaded content.
This explicit projection is application-owned and cannot be checked by the
framework. Use an empty projection when no input is safe to flash.

Browser failures redirect to the same-host Referer path (or `/`); PUT, PATCH,
DELETE and QUERY use 303, other methods use 302. X-Inertia requests redirect
even when they accept JSON. Other JSON requests keep structured JSON errors.
Malformed or unsupported payloads return safe JSON errors for all clients;
only semantic validation failures are redirected.
The `X-Inertia-Error-Bag` header selects an optional bag. Errors use the first
message per field and old input is exposed as `old`. Flash data is removed on
the next request, including unrelated requests; concurrent requests may race,
and there is no automatic reflash across extra redirects or asset-version
reloads. Unlike Laravel, input is not flashed by default. `inertia_rs` 0.4
inserts its default `errors` before shared-prop resolution, so the page-prop
helper is required rather than registering an `errors` shared prop.
Applications may opt into `http::errors::AppError` and register their own Axum
404/405 fallbacks; this does not change existing authentication responses.

## Packages

| Package | Responsibility |
|---|---|
| `aurelin-framework` | Core configuration, HTTP, sessions, cache and locks, authorization, mail, encryption, signed URLs, rate limiting, and authentication primitives |
| `aurelin-fortify` | Headless authentication workflows and Axum integration |
| `aurelin-passkeys` | WebAuthn registration and authentication ceremonies |
| `aurelin` | Application generation and shell completions |

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
cargo run -p aurelin -- completions bash
```

Replace `bash` with another shell supported by `clap_complete`.
