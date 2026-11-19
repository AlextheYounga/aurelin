# Rust crate mapping for Laravel capabilities

**Research snapshot:** 2026-11-18. This began as a research shortlist and now
also records selected dependencies. Selection does not by itself claim Laravel
parity.

**How to read this mapping:** every row names one **leading candidate** (bold)
— the crate to verify first for that capability. A second crate appears only
when it plays a different role in the same solution (for example, a driver
under an abstraction) or when it is an explicitly labeled alternative to a
genuine trade-off. No row leaves the choice ambiguous. Crate existence,
versions, licenses, and key dependencies were checked against crates.io and
docs.rs on the snapshot date; still verify feature flags, MSRV, and runtime
behavior when implementing, and match every behavior against Laravel 13 as
documented in `CONTEXT.md`.

Aurelin's v1 direction is to reuse existing libraries and write only the
integration needed to match the relevant Laravel 13 behavior. This mapping is
intentionally broad because the request is to identify potential wheels to
reuse; it does not mean every row belongs in v1.

## Candidate map

| Laravel capability | Rust candidates | Fit and integration notes |
| --- | --- | --- |
| HTTP server, routes, handlers, extractors, responses | **axum** (with `tower`, `tower-http`, `hyper`) | Axum is the leading candidate: a composable handler/router layer whose Tokio/Hyper/Tower stack is fixed, which keeps the rest of this mapping coherent. Laravel-style route declarations, route names, controller conventions, and URL generation are expected Aurelin-side integration. |
| Middleware, request IDs, timeouts, body limits, CORS, compression | **tower-http** (+ `tower` layers) | Existing Tower layers cover these lower-level concerns (CORS, trace, compression, limits, request IDs, timeouts). Compose only the layers Laravel's documented behavior requires; ordering and CORS policy are application configuration. |
| WebSockets / realtime transport | **axum `ws` extractor**; `socketioxide` only if Socket.IO compatibility is wanted | Axum provides raw WebSocket transport behind a feature. Socketioxide implements the Socket.IO protocol, which is not Laravel broadcasting; adopt it only for that protocol. Broadcasting, channel authorization, and event semantics have no crate in this mapping and need their own research. |
| Database drivers and async SQL | **sqlx** | Async SQL toolkit: pools, prepared statements, optional compile-time checked query macros; PostgreSQL, MySQL/MariaDB, SQLite. Explicitly not an ORM. It is also the layer SeaORM builds on (next row), so this is the driver foundation either way. |
| ORM / Active Record-style data access | **sea-orm**; alternative: `diesel` | Leading candidate because it is async, ActiveRecord-like (entities, relations, eager loading, pagination, migrations), and built on sqlx — the closest match to the Eloquent workflow. Diesel is the alternative if compile-time-checked query DSL matters more than the ActiveRecord shape; it is sync-first. Do not adopt both. |
| SQL query construction | **sea-query** (ships with SeaORM); SQLx `QueryBuilder` for hand-rolled queries | Dynamic SQL for filters and piecemeal construction. With SeaORM selected, sea-query is already present; no third builder needed. |
| Schema migrations | **sea-orm-migration** (pairs with SeaORM); alternatives: `sqlx-cli` (sqlx-only path), `refinery` (framework-agnostic) | Formats and runtime behavior differ per crate; Laravel migration-file compatibility is not established by any of them and must be assessed separately. |
| Factories, seeders, fake data, database test fixtures | **fake** + **`#[sqlx::test]`** / SeaORM `MockDatabase` | Distinct roles: `fake` produces representative values (not persisted factories), `#[sqlx::test]` provisions isolated test databases, SeaORM offers a mock database for query behavior. Factory definitions, persistence, relationship creation, and seeder semantics are expected Aurelin-side conventions. |
| Validation / FormRequest-like validation | **validator** (+ `axum-valid` extractor integration); alternative: `garde` | Leading candidate: derive-based rules with the widest adoption and axum integration via axum-valid. Garde is the alternative (validation context, i18n support). Neither reproduces Laravel's full rule set, message localization, wildcard paths, authorization hooks, or JSON/redirect error shape — verify rule by rule against Laravel 13. |
| Browser sessions and session persistence | **tower-sessions** + **tower-sessions-sqlx-store** or **tower-sessions-redis-store** | Pluggable `SessionStore` with middleware/extractor integration. Note: tower-sessions-redis-store (0.17) is built on **fred ^10**, which fixes the Redis client choice for the whole app (see Cache). Verify cookie attributes, expiration, regeneration, and flash data against Laravel session behavior; PHP-serialized Laravel sessions are not directly interchangeable. |
| Authentication, guards, login state | **axum-login** (built on tower-sessions) | Session-backed authentication with backend traits and route protection. Not a complete Laravel auth starter: registration, password reset, email verification, confirmation, throttling, redirects, and starter screens are Aurelin-side against the Laravel 13 baseline. |
| OAuth / OpenID Connect | **openidconnect** (built on `oauth2`) | The leading protocol crate; wire it into routes and sessions directly. A tiny alpha adapter (`axum-open-id-connect`, no stable release) exists but is not recommended. Optional capability, not part of the auth starter. |
| Password hashing | **bcrypt** for Laravel's default; `argon2` as an explicit alternative | Laravel 13 defaults to bcrypt with cost 12. Aurelin uses the maintained `bcrypt` crate off the async executor for compatible generated defaults. Argon2 may be exposed later as an explicit configured driver, not a silent substitution. |
| Compromised-password validation | **reqwest** + RustCrypto **sha1** | Laravel's production password default uses HIBP's Pwned Passwords range API. No verified dedicated crate fits Aurelin's async stack, MSRV, and licensing: current alternatives introduce a second synchronous HTTP client, require Rust newer than 1.97, or use incompatible licensing. Use SHA-1 only as the HIBP dataset index, send only the five-character prefix with response padding enabled, and compare suffixes locally. |
| Time-based one-time passwords | **totp-rs** 6 | Fortify's default two-factor flow uses RFC 6238 SHA-1, six digits, a 30-second period, configurable skew, single-use replay prevention, QR provisioning, and recovery codes. `totp-rs` 6 is MIT, supports Rust 1.88, secret generation, otpauth URLs, QR generation, constant-time checks, and returns the matched time step needed to reject replay. Fortify remains responsible for workflow, encrypted persistence, confirmation, recovery codes, and rate limiting. |
| Passkeys / WebAuthn | **webauthn-rs** 0.6 + official **@laravel/passkeys** browser package | Preserve Laravel's independent `laravel/passkeys` package boundary as `aurelin-passkeys`, with Fortify integrating its login, confirmation, and management workflows. `webauthn-rs` owns WebAuthn ceremonies and verification; generated applications own users, passkey persistence, relying-party configuration, and pages. Reuse the official Laravel JavaScript client for browser ceremonies rather than creating another client protocol. |
| Authorization (gates/policies) | No crate reproduces Laravel gates/policies; **implement Aurelin-side** over the `axum-login` authenticated-user seam | Gates and policies are app-defined checks over the authenticated user; a thin Aurelin layer matches them more faithfully than a policy engine. Adopt `casbin` (ACL/RBAC/ABAC; explicitly no authentication or user management) only if a real policy-engine requirement emerges. |
| CSRF protection | **axum-tower-sessions-csrf** (synchronizer token, on tower-sessions); stateless alternative: `tower-http` csrf (fetch-metadata via `Sec-Fetch-Site`/`Origin`, no token state) | Two verified approaches, not competing features: the token middleware matches Laravel's synchronizer-token model; tower-http matches the modern fetch-metadata approach. Choose by the behavior to match, plus Inertia/Axios cookie and header expectations. axum-tower-sessions-csrf is MIT OR LGPL-3.0-or-later — check license fit. |
| Cookie parsing, signing, encryption | **cookie** (+ `axum-extra` cookie extractors) | Signed/private jars plus axum extractors. Signing is integrity, not confidentiality. Key generation, rotation, secure attributes, and Laravel cookie interoperability are integration concerns. |
| Cache | **moka** (process-local) + **fred** (Redis/distributed) | Moka covers in-process caching; fred is the Redis client, chosen because tower-sessions-redis-store is fred-based, so cache and sessions share one Redis stack. These are drivers, not a Laravel cache manager: the driver contract, serialization, tags, and atomic locks are a small Aurelin layer. (The `redis` crate would work standalone but would add a second Redis client alongside the session store.) |
| Queue / background jobs / retries | **apalis** (+ backend crates); Postgres-only alternative: `sqlxmq` | Apalis: worker processing across Redis, PostgreSQL, SQLite, and other backends, with retries and tower middleware — the closest match to Laravel's multi-driver queue model. Compare delivery guarantees, failure handling, scheduling, and operational tooling with Laravel queues before selecting. |
| Scheduled tasks / cron | **apalis-cron** (pairs with apalis); alternative: `tokio-cron-scheduler` | Cron scheduling integrated with apalis workers and persistence options. Laravel scheduler semantics (without-overlapping, on-one-server, mutexes, output handling) are not provided by either — verify and implement where required. |
| Events (dispatcher) | No crate reproduces Laravel's event dispatcher; primitives: **`event-listener`** (sync listeners), tokio `broadcast` (async fan-out) | A Laravel-style dispatcher (event discovery, queued listeners, wildcard events, cross-process delivery) is thin Aurelin-side code over these primitives when needed; queued delivery rides the selected queue crate. |
| Notifications (multi-channel) | No crate; compose **lettre** (email channel) + the selected queue crate | Lettre and the queue cover async email delivery, which is the v1-relevant channel. SMS, database, and broadcast channels, plus per-user channel preferences, need their own research before any implementation. |
| Mail composition and SMTP delivery | **lettre** | Message building and pluggable SMTP/sendmail/file transports, including async support. Mailer configuration management, templates, and queue integration are Aurelin-side. |
| File uploads | **axum-typed-multipart** (on axum `Multipart`) | Typed multipart extraction with per-field size limits; raw `Multipart` is available behind an axum feature. |
| Storage (local/object disks) | **object_store**; S3-only alternative: `aws-sdk-s3` | A common async API across local and cloud stores — the closest analog to Laravel's multi-disk filesystem. Upload limits, streaming, path safety, URL generation, and visibility are Aurelin-side. |
| Static files and built frontend assets | **tower-http `ServeDir`** (filesystem) and **rust-embed** (embedded) | Two roles, not competitors: serving from disk vs embedding assets in the binary. Neither reproduces Laravel Vite's manifest, dev-server, hot-file, and versioning conventions (next row). |
| Vite dev server / build manifest | **vite-rs** (0.3) | Verified description: compiles frontend assets through ViteJS, serves from the Vite dev server in development, and embeds built assets into the binary for production. It does not produce Laravel-style `@vite` manifest tags; if that exact behavior is required, a small manifest reader may be simpler than adapting it. |
| Inertia protocol / Vue and React integration | **inertia_rs** 0.4 with the `axum` feature; alternatives: `inertia-omega` (official org, alpha), `axum-inertia` (protocol version unverified) | inertia_rs implements the Inertia v3 server protocol (partial reloads, merge/deferred/once props, history encryption, shared props, external redirects) on a framework-neutral core with an Axum adapter; SSR and async prop resolvers are not fully implemented (MSRV 1.88, MIT). inertia-omega is the official-org port, explicitly alpha. axum-inertia (0.9, last release 2025-07) predates both — verify its protocol version before considering. The crates.io `inertia-rust` targets a different, older protocol major and is excluded. Inertia's official Vue/React client packages and Vite remain JavaScript dependencies. |
| Configuration and environment | **config** + **dotenvy**; alternative: `figment` (profile-oriented) | `config` layers typed file/env sources with explicit precedence; dotenvy loads `.env` input. Figment is the alternative if Rocket-style profiles are preferred. dotenvy does not validate application settings. |
| JSON, form, query, serialization | **serde** + **serde_json** (with axum extractors) | The ecosystem's serialization foundation; axum supplies JSON, form, path, and query extraction with rejection types. Laravel's request casting and response formatting are behavior mapping, not crates. |
| Outbound HTTP client | **reqwest** | The high-level async HTTP client. Laravel's configured-client conveniences (retry defaults, request macros, fakes) are Aurelin-side. |
| Localization / translation messages | **rust-i18n**; ICU-grade alternative: `fluent-bundle` | rust-i18n follows the Rails/Laravel-style workflow: locale files (YAML/JSON/TOML), a global `t!` macro, fallback locale chains. Fluent offers ICU message formatting if that fidelity is required. Either way, verify pluralization, locale negotiation, and validation/auth message integration against Laravel 13. |
| Rate limiting | **governor** (primitive) + **tower-governor** (tower/axum layer) | Per-key limits (peer IP, headers, custom keys), process-local unless backed by shared storage. Check distributed needs and Laravel throttle semantics (per-user keys, decay minutes, headers, response behavior) before adoption; trust proxy-derived IPs only when proxy configuration is known. |
| Encryption (Laravel Crypt) and signed URLs | **RustCrypto primitives** (`aes-gcm`, `sha2`/`hkdf`) | No crate provides Laravel's encrypted-payload format; the Aurelin encrypter (AES-GCM, APP_KEY-derived keys, key rotation) is expected to be implemented over these maintained primitives. Signed URLs are an Aurelin-side construction over HMAC-based signatures. |
| API schema and generated documentation | **utoipa** (+ `utoipa-axum`, `utoipa-swagger-ui`) | OpenAPI derived from Rust types/handlers; utoipa-axum can collect routes while generating a spec. Optional capability, not required for the Laravel application workflow. |
| Pagination | **SeaORM paginator/cursor APIs**; alternative: `paginator-rs` family (`paginator-sqlx`, `paginator-sea-orm`, `paginator-axum`) | SeaORM pagination aligns with the ORM choice. The paginator family (page/offset/cursor, filtering) is newer — validate maturity, count queries, stable ordering, and Laravel response shape before considering it. |
| Logging, tracing, metrics, distributed tracing | **tracing** + **tracing-subscriber** + tower-http trace layer; export via **opentelemetry** / **tracing-opentelemetry** | Complementary pieces with version/feature alignment requirements; not a Laravel Telescope replacement. |
| Error reporting | **sentry** (optional) | Hosted error/performance reporting SDK with tracing integration. Service-specific; keep optional, never a framework requirement. |
| CLI parsing, prompts, progress, scaffolding | **clap**; **dialoguer** (prompts); **indicatif** (progress); **minijinja** / **include_dir** (templates) | Distinct roles in one artisan-like CLI. Cargo already provides package/workspace operations; template generation should not reimplement Cargo behavior. |
| Application errors | **thiserror** (typed errors) + **anyhow** / **miette** (CLI and app diagnostics) | Roles are distinct: typed errors at library boundaries, context-rich diagnostics for the CLI. Laravel-compatible HTTP error rendering is Aurelin-side. |

## Leading candidates (verify these first)

1. **axum + tower + tower-http** — HTTP/Tower foundation.
2. **sea-orm (on sqlx)** — data layer; fall back to sqlx alone if no ORM is
   wanted.
3. **tower-sessions + axum-login + bcrypt** — authentication building blocks;
   the complete Laravel 13 starter behavior is Aurelin-side.
4. **validator + axum-valid** — request validation.
5. **inertia_rs (axum feature)** — Inertia v3 server protocol; run a
   compatibility spike against the official Vue/React client packages first.
6. **apalis + apalis-cron** — queued jobs and scheduled tasks.
7. **fred + moka** — distributed and process-local cache; fred also underlies
   the Redis session store.
8. **lettre** — mail composition and delivery.

## Gaps and cautions

- A crate feature is not automatically Laravel-equivalent. Preserve Laravel 13
  defaults, success/failure behavior, status codes, redirects, validation
  shape, and security behavior when translating a requested capability.
- Several capabilities have no crate and are expected Aurelin-side: the event
  dispatcher, gates/policies, the notification framework beyond email, the
  encrypter payload format, and the artisan-style console itself. The listed
  primitives are the building blocks, not substitutes.
- Authentication starter kits are a composed experience, not just login
  middleware. The baseline remains the built-in authentication variant of the
  Laravel 13 official Vue/React starter kits documented in `CONTEXT.md`.
- Governor and tower-governor use replenishing GCRA/token-bucket semantics,
  while Laravel's login limiter uses a fixed decay window and reports its
  remaining retry delay. Aurelin's small fixed-window adapter is a documented
  deviation for that behavior; general HTTP throttling should still verify
  Governor first.
- Do not select both options within a row (two ORMs, two validation libraries,
  two Redis clients, two Inertia adapters) without a demonstrated integration
  requirement; the bold candidate is the one to verify first.
- Framework-specific adapters assume the axum/tower stack. Actix, Rocket, and
  other runtimes would require different crates or adapters.
- Check crate licenses against the project's distribution plans. The list
  includes differing licenses (for example, MIT OR LGPL-3.0-or-later on one
  CSRF crate) and should not be treated as pre-cleared.
- This list is intentionally not a promise to implement every Laravel feature.
  Research each requested capability against Laravel 13's documentation/source
  and the candidate's authoritative docs before adding dependencies.

## Sources

Primary candidate documentation and crate pages (accessed 2026-11-18):

- [Axum](https://docs.rs/axum/latest/axum/), [Tower HTTP](https://docs.rs/tower-http/latest/tower_http/), [Tower](https://docs.rs/tower/latest/tower/)
- [SQLx](https://docs.rs/sqlx/latest/sqlx/), [SeaORM](https://www.sea-ql.org/SeaORM/), [Diesel](https://docs.rs/diesel/latest/diesel/), [SeaQuery](https://docs.rs/sea-query/latest/sea_query/), [Refinery](https://docs.rs/refinery/latest/refinery/)
- [Axum Login](https://docs.rs/axum-login/latest/axum_login/), [Tower Sessions](https://docs.rs/tower-sessions/latest/tower_sessions/), [bcrypt](https://docs.rs/bcrypt/latest/bcrypt/), [Argon2](https://docs.rs/argon2/latest/argon2/), [Casbin](https://casbin.apache.org/docs/rust-rbac-tutorial/)
- [Apalis](https://docs.rs/apalis/latest/apalis/), [Apalis Cron](https://docs.rs/apalis-cron/latest/apalis_cron/), [SQLxMQ](https://docs.rs/sqlxmq/latest/sqlxmq/)
- [Lettre](https://docs.rs/lettre/latest/lettre/), [object_store](https://docs.rs/object_store/latest/object_store/), [Moka](https://docs.rs/moka/latest/moka/), [fred](https://docs.rs/fred/latest/fred/)
- [inertia_rs repository](https://github.com/stuarth/inertia-rs), [Inertia Omega repository](https://github.com/inertiajs/inertia-omega), [Inertia protocol](https://inertiajs.com/the-protocol)
- [Garde](https://docs.rs/garde/latest/garde/), [Axum Valid](https://docs.rs/axum-valid/latest/axum_valid/), [Utoipa](https://docs.rs/utoipa/latest/utoipa/), [SQLx test](https://docs.rs/sqlx/latest/sqlx/attr.test.html)
- [Tower Governor](https://docs.rs/tower-governor/latest/tower_governor/), [figment](https://docs.rs/figment/latest/figment/), [config](https://docs.rs/config/latest/config/), [dotenvy](https://docs.rs/dotenvy/latest/dotenvy/)
- [rust-i18n](https://docs.rs/rust-i18n/latest/rust_i18n/), [fluent-bundle](https://docs.rs/fluent-bundle/latest/fluent_bundle/), [Vite](https://vite.dev/)
- [Laravel 13 starter kits](https://laravel.com/framework/docs/13.x/starter-kits), [Laravel 13 authentication](https://laravel.com/framework/docs/13.x/authentication)
- Existence, version, license, and dependency checks via the crates.io API
  (2026-11-18): `axum-tower-sessions-csrf` 0.1.4, `axum-inertia` 0.9.0,
  `inertia_rs` 0.4.0, `paginator-axum` 0.4.0, `tower-sessions-redis-store`
  0.17.0 (fred ^10), `vite-rs` 0.3.0, `axum-governor` 2.1.2,
  `axum-open-id-connect` 0.2.0-alpha.9.
