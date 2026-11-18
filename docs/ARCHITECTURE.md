# Architecture

## Repository layout

Kiln is a Rust framework ecosystem modeled on Laravel. Independently usable
packages live under `crates/` and may eventually be published separately.

```text
kiln/
├── Cargo.toml
├── CONTEXT.md
├── crates/
│   ├── kiln-framework/       # Runtime integration and conventions
│   └── kiln-cli/             # Project generator and embedded starters
│       └── starters/base/
└── laravel -> ...            # Local Laravel source reference, not a dependency
```

## Implemented foundation

### Application creation

`kiln new <path> --framework-path <path>` delegates package creation and name
validation to `cargo new`, then installs Kiln's starter files. It refuses an
existing destination rather than adopting or overwriting it. The framework
path is temporary until `kiln-framework` is published.

The generated application separates its concerns in the same broad order as a
Laravel application:

- `src/main.rs` loads configuration and owns the server process;
- `src/bootstrap.rs` composes the application;
- `src/routes/http.rs` declares HTTP routes;
- `.env.example` documents environment-specific settings.

This is a Rust translation, not a copy of PHP's runtime or service container.
Axum owns routing, handlers, middleware composition, and HTTP serving. Tokio
owns the runtime, listener, and graceful Ctrl+C shutdown. Kiln does not wrap
those APIs where a wrapper adds no behavior.

### Configuration

The framework reads `.env` with `dotenvy` and applies external variables last,
so deployment configuration wins. An externally supplied `APP_ENV` selects an
existing `.env.APP_ENV`; a value found inside `.env` does not retroactively
select another file. Missing files are allowed. Loading creates an owned
snapshot instead of mutating process-wide environment variables, which avoids
unsafe global mutation in a concurrent Rust application.

`config` deserializes the snapshot into typed settings. The first settings are
Laravel's development server defaults: `SERVER_HOST=127.0.0.1` and
`SERVER_PORT=8000`. Invalid and out-of-range ports fail before binding.

### HTTP lifecycle

The generated process binds the configured listener, serves the composed Axum
router, and exits cleanly after Ctrl+C. Bind errors and configuration errors
produce a failing process status. The starter includes:

- `GET /`, returning the welcome response;
- `GET /up`, returning an up response and JSON `{ "status": "up" }` when JSON
  is requested;
- Axum's native 404 and method-not-allowed behavior.

The health route currently establishes only the default healthy response. Kiln
does not yet have Laravel's event dispatcher, so it cannot yet reproduce the
`DiagnosingHealth` hook or its failure path.

### Inertia protocol

`kiln-framework` re-exports `inertia_rs` 0.4 with its Axum adapter. Integration
tests exercise Inertia 3 initial HTML visits, JSON visits, script-safe page
data, asset versions, and stale-version reload responses through a real Axum
router.

This is not a claim of complete protocol parity. The selected adapter documents
SSR and asynchronous prop resolvers as unsupported, and validation error bags
as partial. Those gaps must be addressed or reassessed when their associated
starter behavior is implemented. The official `inertiajs/inertia-laravel-rust`
port was also inspected at its 2026-11-18 `main` branch: it is alpha,
unpublished to crates.io, and requires Rust 1.99, so it is not a compatible v1
dependency for the workspace's current Rust 1.97 toolchain.

### Vue and React frontend foundations

`kiln new` accepts `--frontend react` or `--frontend vue`. Both variants use
the same Axum routes and Inertia responses, and differ only in their Vite
plugin, entry point, and page component. The versions verified by the starter
are Inertia client/Vite plugin 3.11, React 19, Vue 3.5, TypeScript 5.9, and Vite
8. The generated lock files make those checks reproducible.

`axum-vite` 0.3.4 supplies the integration that Kiln would otherwise have to
write: Vite proxying and HMR in development, React's refresh preamble, manifest
entry resolution, immutable production asset serving, and embedding `dist` in
the release binary. It is MIT OR Apache-2.0 and uses Axum 0.8. Kiln adds only
the Inertia root document and asset version wiring. A generated app starts Vite
as a managed child in debug builds and terminates it when the backend exits.
Production follows Laravel's build-before-deploy workflow: `npm run build`
must precede `cargo build --release`.

### Authentication foundation

Both frontend variants now share real registration, login, logout, protected
dashboard, and password-reset routes and screens. SQLx owns the initial SQLite
schema and parameterized user queries. `axum-login` 0.18 provides authenticated
sessions and session-ID rotation, `tower-sessions` 0.14 plus the exactly pinned
compatible SQLx store persists sessions, Argon2id hashes passwords off the
async executor, and `axum-tower-sessions-csrf` 0.1.1 validates state-changing
requests. Login failures are limited to five per minute per lowercased email
and client IP, matching Laravel's default key and threshold.

Password-reset tokens are random, stored only as SHA-256 digests, expire after
60 minutes, and are consumed transactionally with the password update. The
current development notification path logs the reset URL. A production mail
transport, email verification, password confirmation, and remaining official
starter behavior are not implemented, so this remains a partial authentication
starter rather than a parity claim.

Generated applications keep reusable behavior in a library target and a thin
binary entry point. All Rust tests live in each crate's root `tests/` directory;
production `src/` trees contain no inline test modules or test-only seams.

## Behavior references

- Laravel framework checkout: `v13.40.0-3-gddbf5ebcde`, revision
  `ddbf5ebcdef2be42d326c8c909c9e40228d9e517`.
- Laravel application skeleton `13.x`: revision
  `a255d7688770af9713daff1439c47ff9bc6abd47` (2026-11-13).
- Laravel 13 request lifecycle, routing, configuration, and starter-kit docs,
  accessed 2026-11-18.
- Official Vue starter: revision
  `1959e1a26b1cabd02037c30210e14c5752d2b431`.
- Official React starter: revision
  `012f4248f70e255e32ad93f540aca5fb7e8c96fe`.
- `inertia_rs` 0.4.0 protocol support matrix and Axum source.

## Growth rule

Add dependencies and framework behavior only for the capability currently
being implemented. Verify Laravel behavior and the selected library first.
Prefer direct use of existing Rust and frontend APIs; add Kiln code only for a
real integration or convention gap. A selected dependency is not evidence of
Laravel parity.

## Validation

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The test suite creates a project in a temporary directory, compiles and tests
that generated project, starts its actual binary on an ephemeral port, sends
HTTP requests, verifies failure paths, and exercises graceful shutdown.
