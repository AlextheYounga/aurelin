# Architecture

## Repository layout

Aurelin is a Rust framework ecosystem modeled on Laravel. Independently usable
packages live under `crates/` and may eventually be published separately.

```text
aurelin/
├── Cargo.toml
├── CONTEXT.md
├── crates/
│   ├── aurelin-framework/       # Runtime integration and conventions
│   ├── aurelin-fortify/         # Headless authentication package
│   ├── aurelin-passkeys/        # WebAuthn ceremony integration
│   └── aurelin/                 # Project generator and embedded starters
│       └── starters/base/
└── laravel -> ...            # Local Laravel source reference, not a dependency
```

## Implemented foundation

### Application creation

`aurelin new <path> --framework-path <path>` delegates package creation and name
validation to `cargo new`, then installs Aurelin's starter files. It refuses an
existing destination rather than adopting or overwriting it. The framework
path is temporary until `aurelin-framework` is published.

The generated application separates its concerns in the same broad order as a
Laravel application:

- `src/main.rs` loads configuration and owns the server process;
- `src/bootstrap.rs` composes the application;
- `src/routes/http.rs` declares HTTP routes;
- `.env.example` documents environment-specific settings.

This is a Rust translation, not a copy of PHP's runtime or service container.
Axum owns routing, handlers, middleware composition, and HTTP serving. Tokio
owns the runtime, listener, and graceful Ctrl+C shutdown. Aurelin does not wrap
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

The health route currently establishes only the default healthy response. Aurelin
does not yet have Laravel's event dispatcher, so it cannot yet reproduce the
`DiagnosingHealth` hook or its failure path.

### Inertia protocol

`aurelin-framework` re-exports `inertia_rs` 0.4 with its Axum adapter. Integration
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

`aurelin new` accepts `--frontend react` or `--frontend vue`. Both variants use
the same Axum routes and Inertia responses, and differ only in their Vite
plugin, entry point, and page component. The versions verified by the starter
are Inertia client/Vite plugin 3.11, React 19, Vue 3.5, TypeScript 5.9, and Vite
8. The generated lock files make those checks reproducible.

`vite-rs` 0.3 and its Axum 0.8 adapter provide the mapped integration: a
managed Vite development server, Tower asset service, release builds, and
embedded production output. Aurelin supplies the dynamic Inertia root document,
React refresh preamble in development, and asset-version wiring. A generated
app starts Vite as a managed child in debug builds and terminates it when the
backend exits. During `cargo build --release`, `vite-rs` runs Vite and embeds
the resulting `dist` files into the executable.

### Rust/WASM frontend foundation

`aurelin new --frontend wasm` generates a separate full-stack Leptos 0.8
application convention. Leptos renders route-specific HTML through Axum,
hydrates the existing document in the browser, and owns browser navigation and
typed server-function dispatch. The starter uses Leptos APIs directly; Aurelin
does not add an Inertia page protocol or rename ordinary Leptos concepts.

`cargo-leptos` 0.3 builds the native server and the
`wasm32-unknown-unknown` browser library. Server-only Aurelin, Tokio, SeaORM,
and CLI dependencies are optional and enabled only by the `ssr` feature, so
the hydration target does not compile the backend stack. The generated project
contains no package manifest, Vite configuration, npm step, or Node.js
dependency. Axum infrastructure routes such as `/up` remain separate from the
Leptos presentation routes.

This follows Laravel Blade's ownership boundary: Laravel Framework owns its
reusable view integration while applications own templates. Aurelin's
`aurelin-framework::view::leptos` module therefore owns reusable Leptos request
protections, currently synchronizer-token issuance and validation for server
functions. Generated applications continue to own their Leptos shell, routes,
components, and ordinary router composition.

The WASM starter establishes generation, SSR, hydration, routing, typed server
functions, static assets, the Node-free production build, persisted sessions,
and Fortify-backed registration, login, logout, and authenticated dashboard
boundaries. Those server functions delegate password policy, user creation,
verification notification, authentication timing, throttling, session
transitions, remember-me behavior, and two-factor branching to
`aurelin-fortify`; they do not copy those workflows into the starter. The
remaining email-verification action, password-reset, two-factor input, passkey,
and account-settings presentation adapters are incomplete.

### Authentication foundation

The behavioral baseline was rechecked on 2026-11-19 against Laravel Framework
13.x, Fortify commit `516753a645ee9e6adc68241ee7825f0ec563a0a5`, React
starter commit `012f4248f70e255e32ad93f540aca5fb7e8c96fe`, and Vue starter
commit `1959e1a26b1cabd02037c30210e14c5752d2b431`. The comparison covered
routes and methods, Inertia props, redirects, validation responses, and the
authentication security pipeline. Aurelin tests the same public scenarios
through Rust boundaries rather than copying PHP internals.

Authentication follows Laravel's package boundary. `aurelin-framework` owns
framework primitives: bcrypt hashing with Laravel's default cost, password
reset token lifecycle, fixed-window rate limiting, temporary signed URLs, and
Laravel-compatible session defaults. `aurelin-fortify` owns Fortify's
application action contracts, login-rate-limit convention, and TOTP workflow.
`aurelin-passkeys` owns WebAuthn ceremonies behind Laravel's separate passkeys
package boundary. Generated
applications provide user persistence, migrations, action implementations,
route/view selection, and frontend pages. The CLI only installs those packages
and application-owned templates.

Both frontend variants share registration, login, logout, remember-me,
protected dashboard, password-reset, email-verification,
password-confirmation, profile, account-deletion, appearance, and a unified
security screen for password, two-factor, and passkey management. SeaORM 1.1 provides application
data access, SeaQuery-backed `sea-orm-migration` owns the SQLite schema, and
SQLx remains SeaORM's driver and the Tower session-store infrastructure.
`axum-valid` integrates `validator` with Axum request extraction. `axum-login`
0.18 provides authenticated sessions and session-ID rotation, and
`axum-tower-sessions-csrf` 0.1.1 validates state-changing requests.

Password-reset tokens are random, stored only as SHA-256 digests, expire after
60 minutes, and are deleted after Fortify invokes the generated application's
password-reset action. The broker preserves Laravel's sent, unknown-user,
throttled, invalid-token, and successful-reset outcomes. Email verification
uses Fortify-owned temporary signed URLs backed by `APP_KEY` instead of
application token storage. Login requests use the starter's five-per-minute
email-and-IP limit; verification requests use Fortify's six-per-minute user
limit; passkey ceremonies use the starter's ten-per-minute credential-or-session
and IP limit. All return HTTP 429 and Laravel's rate-limit headers when exhausted.
Only failed credentials consume the login limit, and an ordinary successful
login clears earlier failures. Failed credential checks are timeboxed to
Fortify's configurable 200 ms default for both ordinary and pre-two-factor
login, while successful checks return as soon as password verification finishes.
Successful credential validation also replaces bcrypt hashes whose cost no
longer matches the application's configured default before the session or
two-factor challenge continues.

Login, registration, and password-reset routes enforce Fortify's guest-only
boundary for both views and submissions. Password confirmation uses Fortify's
`/user/confirm-password` route, verifies the current password, and records the
confirmation time in the session. Local and test environments require
eight-character passwords. Production also requires 12 characters, mixed case,
a number, and a symbol, and supplies the same browser `passwordrules` hints as
the starter. Production also queries HIBP's padded range API with only the
five-character SHA-1 prefix; lookup failures do not prevent password selection.

Two-factor secrets and recovery codes use authenticated application-key
encryption. The Fortify flow includes setup confirmation, QR and secret access,
single-use recovery codes, TOTP replay rejection, and the five-per-minute login
challenge limit. Passkeys use `webauthn-rs` for user-verified discoverable
credentials, opaque application-specific user handles, server-held ceremony
state, and persisted credential-counter updates. React and Vue call the official
`@laravel/passkeys` client against Laravel's route contract.

The log mailer remains the local default. Configuring `MAIL_MAILER=smtp` uses
`lettre` for authenticated TLS, STARTTLS, or explicitly unencrypted local SMTP
delivery. Registration and verification requests send signed verification
links, while password-reset requests send the broker's expiring reset link.
Remember-me extends the opaque server-side session to Laravel's five-year
remembered-login lifetime; password changes invalidate other sessions through
the authenticated user's password-hash session fingerprint.

The generated integration suite follows the observable scenarios in Laravel's
starter tests rather than porting PHP implementation assertions. It exercises
the real Axum/Inertia boundary and persisted SQLite state for profile updates,
email normalization and uniqueness, verification state, password updates and
throttling, account deletion, session invalidation, remember-me cookies, and
SMTP delivery. Remaining parity differences must be reported precisely rather
than treating this as a blanket complete-starter claim.

The targeted comparison leaves these explicit authentication boundaries:

- authentication events remain deferred until Aurelin has a general event
  dispatcher, as tracked in the roadmap;
- all default authentication features are currently generated together;
  Laravel's feature-selective starter generation is pending a product decision;
- Aurelin currently targets the official Inertia starter workflow, not
  Fortify's complete content-negotiated JSON API, configurable route paths,
  guards, domains, and middleware surface;
- authentication and validation text is currently fixed English text pending
  the general localization milestone;
- full browser WebAuthn qualification remains deferred to the reliable browser
  test setup recorded in the roadmap.

Generated applications keep reusable behavior in a library target and a thin
binary entry point. All Rust tests live in each crate's root `tests/` directory;
production `src/` trees contain no inline test modules or test-only seams.

### Authorization

Laravel Framework owns gates, authorization responses, policies, and its `can`
middleware. Aurelin therefore keeps the reusable mechanics in
`aurelin-framework::authorization`. A cloneable `Gate` registry evaluates
application-defined abilities for the current `axum-login` user, denies guests
unless an ability explicitly accepts them, and preserves policy messages,
reason codes, default 403 responses, and deliberate 404 concealment. Global
before hooks may resolve a check before its ability; after hooks observe every
resolved result and may only supply a result when no earlier check did.

Applications own their policy structs, dependencies, resource types, ability
names, and registration. The framework's `Policy` seam only installs those
application rules into the shared gate registry. Registration is explicit
rather than discovered from PHP-style namespaces or reflection. Ability
contexts are typed, can be tuples or application context structs when more than
one value is needed, and mismatched context types are treated as application
configuration errors rather than ordinary denials.

`AuthorizeLayer` performs resource-free or fixed-context checks before an Axum
route handler and reads the user installed by `AuthManagerLayer`. Resource
handlers load their models through Axum and SeaORM before calling
`gate.for_user(...).authorize(...)`; Aurelin does not introduce an implicit
Eloquent-style model-binding layer. Inline `allow_if` and `deny_if` checks bypass
registered hooks as they do in Laravel. Gate evaluation events remain deferred
until Aurelin's general event dispatcher exists, policy generation belongs to
the later CLI-tooling milestone, and authorization props for Inertia remain an
application presentation decision as Laravel documents.

### Cache foundation

Laravel Framework's `illuminate/cache` package owns the cache repository,
stores, and lock contracts. Aurelin keeps the corresponding reusable behavior
in `aurelin-framework::cache`. Generated applications select a driver and own
its environment and service configuration, including the Moka capacity, Redis
URL, Redis database, and key prefix. Starter templates do not contain cache
algorithms or lock scripts.

The cloneable `Cache` API provides typed `get`, `put`, `put_many`, `get_many`,
`forever`, `forget`, `flush`, `has`, `missing`, `add`, `pull`, integer mutation,
`touch`, `remember`, and `remember_forever` operations. Positive lifetimes are
whole seconds; zero and negative write lifetimes remove an existing value,
while conditional `add` performs no write. Missing integer mutations create an
indefinite signed integer. Moka 0.12.16 supplies process-local storage and Fred
10.1.0 supplies Redis or Valkey storage. Driver and connectivity failures are
returned rather than converted into misses.

Both drivers store non-null JSON. Decoding is requested as a concrete Rust
type, so malformed data and type mismatches are explicit errors. JSON was
chosen as a safe data representation, not for PHP serialization
interoperability. Keys receive the application's explicit prefix. Like
Laravel's Redis warning, `flush` ignores that prefix and clears the selected
backend (the whole Moka store or Redis database).

Atomic locks use independent opaque 256-bit random owner tokens. Acquisition,
owner-aware release, force release, restoration, refresh, expiration, and
blocking timeouts are supported. Moka serializes local operations through its
atomic entry API. Redis uses `SET NX` with expiration and owner-comparing Lua
scripts for release and refresh; a handle cannot release another owner's lock.
Blocking deadlines use Tokio's monotonic clock. Scheduling may later consume
this lock API for overlap prevention, but scheduling behavior is outside the
cache module and remains unimplemented.

This is a deliberate Laravel 13 subset. Tags, stale-while-revalidate,
memoization, cache events, failover, custom driver registration, a multiple
named-store manager, and PHP serialization compatibility remain deferred.

### Application console

Generated projects expose their application commands through the visible Cargo
alias `cargo aurelin`. The application-owned registry lives in `src/console/`
because it selects that application's migrations and custom commands. Reusable
migration execution lives in `aurelin-framework` and uses the mapped
`sea-orm-migration` stack. Running `cargo aurelin` lists only the generated
application's commands; `dev`, `serve`, migration commands, and `queue:work`
remain explicit operations. This follows Laravel 13's Artisan command and
migration workflows while using static Rust command registration.

### Queue foundation

Laravel Framework owns queue dispatch and worker behavior, so Aurelin keeps its
reusable queue integration in `aurelin-framework`. The initial integration uses
the mapped Apalis stack with `apalis` 1.1.1 and `apalis-sqlite` 1.1.1. The
framework prepares Apalis's durable SQLite storage and exposes its native job
sink and worker types rather than inventing a second queue API.

Generated applications own their serializable job types and handlers. The
starter includes an example job, a dispatch function, and a `cargo aurelin
queue:work` process using the same `DATABASE_URL` as the application. Generated
application tests dispatch through SQLite and process the job through a real
worker. Queue tables are application-owned migrations, so dispatch and workers
fail with migration guidance instead of creating schema during startup. Apalis
persists attempts and terminal failures in its durable job rows; Aurelin exposes
those rows through `queue:failed`, `queue:retry`, `queue:forget`, and
`queue:flush`. This deliberately avoids duplicating each terminal job into a
second Laravel-shaped `failed_jobs` table: retry resets the durable row before a
worker claims it again. Per-job backoff, worker timeouts, additional queue
drivers, batching, and chaining remain future parity work.

## Growth rule

Add dependencies and framework behavior only for the capability currently
being implemented. Verify Laravel behavior and the selected library first.
Prefer direct use of existing Rust and frontend APIs; add Aurelin code only for a
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
