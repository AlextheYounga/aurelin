# Aurelin Code Placement

Decide ownership before implementation. Aurelin is a Laravel 13 translation
assembled from existing Rust libraries; a working generated-app scenario alone
does not prove that behavior belongs in the starter.

## Placement gate

For each capability, establish and state:

```text
Laravel owner (package/source):
Aurelin owner (crate/module):
Application-owned seam:
Generated-file effect (if any):
```

Check the Laravel package boundary and behavior, then `CONTEXT.md`, the relevant
row and notes in `docs/crate-mapping.md`, `docs/ARCHITECTURE.md`, crate public
modules/manifests, tests, and the scaffold path. Use the mapping's leading
candidate and preserve distinct layers; SQLx is SeaORM's driver, not a reason to
replace SeaORM for application data access. Verify mapped APIs and constraints
before adopting them. If ownership or the mapping is unsettled, inspect and
record the gap or ask for a decision—do not settle it by placing code in a
starter or choosing an unmapped alternative silently.

## Ownership and dependency direction

- **`crates/aurelin-framework/` / `aurelin_framework`**: reusable integrations
  and Laravel Framework primitives. Public capabilities live under focused
  `src/` modules and are exposed from `src/lib.rs`; use existing module seams
  where they fit. Examples: HTTP/session/auth primitives in
  `src/auth/` and `src/session.rs`, generic cache in `src/cache/`, generic mail
  and notifications in `src/mail.rs` and `src/notifications.rs`, and reusable
  authorization in `src/authorization/`. Prefer the mapped library directly;
  Aurelin code belongs here only for a real Laravel behavior or integration
  gap, not as a wrapper around Axum or another API without added value.
- **Focused package crate**: behavior owned by an independently packaged
  Laravel product. `aurelin-fortify` owns Fortify workflows, action contracts,
  and Axum integration (`crates/aurelin-fortify/src/`); `aurelin-passkeys`
  owns WebAuthn ceremony mechanics (`crates/aurelin-passkeys/src/lib.rs`).
  Framework primitives remain below Fortify. Package code may depend on lower
  framework packages and accept application adapters, but must not depend on
  the CLI or starter files.
- **`crates/aurelin/` CLI**: command parsing, generation choices, dependency
  declarations, file extraction, and explicit transformations only. Runtime
  behavior does not belong in `src/main.rs` or `src/scaffold.rs` merely because
  the CLI installs it.
- **Generated application**: application-owned models/entities, migrations,
  action implementations, policy rules, provider and environment configuration,
  package selection, route/view selection, callbacks/listeners/jobs, and
  presentation. Keep bootstrap and handlers thin: compose/register, validate or
  authorize, delegate, return. Examples: user persistence and action adapters
  under `crates/aurelin/starters/authentication/src/`; application routes and
  bootstrap under `starters/base/src/routes/` and `bootstrap.rs`; UI in
  `starters/react/`, `starters/vue/`, or `starters/wasm/`.

Keep dependencies flowing from generated application → focused Aurelin package
→ framework/lower-level dependency. Neither framework nor package crates may
reach upward into generated application code or the CLI. Application registration
is explicit (for example, policy/listener registration or `src/schedule.rs`),
not implicit discovery. Persistence schemas and domain records stay with the
application; reusable persistence mechanics/integration stay in their owner
crate. External-service configuration, credentials, disk choice, and app
authorization stay application-owned; reusable transport/protocol mechanics
belong in the mapped integration/package.

## Modules, crates, tests, and templates

- Extend a focused existing module when the behavior shares its owner and
  responsibility. Split a module when it is accumulating a distinct concept or
  responsibility (the framework's `cache/` and `authorization/` are examples).
  Create a crate only for a meaningful package/independent ownership boundary,
  not simply to make a file smaller. Follow nearby public API and error patterns.
- Test reusable public behavior in its owning crate's root `tests/` directory
  (e.g. `crates/aurelin-framework/tests/authorization.rs` or
  `crates/aurelin-fortify/tests/actions.rs`). Test composition and generated
  behavior through real boundaries in `crates/aurelin/tests/` and the relevant
  starter `tests/` (e.g. `crates/aurelin/tests/generated_application.rs` and
  `crates/aurelin/starters/frontend/tests/authentication.rs`). Keep production
  `src/` free of inline test modules; test observable behavior, not private
  implementation details.
- Starter files under `crates/aurelin/starters/` are compiled into the CLI via
  `include_dir` and extracted into each new project by `crates/aurelin/src/scaffold.rs`.
  Treat every extracted behavior as a permanent fork: fixes will not reach
  already-generated applications. Put only application-owned code or the
  smallest adapter/configuration needed to use a package there. Never copy a
  reusable workflow, package controller/middleware/default/security rule, or
  behavior solely to make an end-to-end starter test pass. Improve the package
  seam first, then add the thin app adapter and generated-app test.

## Common placement failures to avoid

- Implementing Fortify authentication behavior in
  `starters/authentication/src/auth/` instead of `aurelin-fortify`.
- Putting reusable cache algorithms, mail transport, queue/scheduler runtime,
  or HTTP protocol behavior in starter files or the CLI.
- Putting application-specific models, schema migrations, policies, route
  selection, provider credentials, or UI in a generic framework crate.
- Treating `aurelin-framework` as the owner of every first-party capability;
  package ownership still follows Laravel boundaries (notably Fortify and
  passkeys).
- Using SQLx directly as the application ORM, or adding an alternative mapped
  dependency just because it is already transitive/familiar. Follow
  `docs/crate-mapping.md`; deviations require a verified gap, explicit decision,
  and mapping update when the decision changes the research.

When a capability does not clearly fit these established boundaries, inspect
its Laravel owner, Aurelin call graph, generated-file behavior, and current
tests, then document the ambiguity and get a placement decision rather than
inventing a convention.
