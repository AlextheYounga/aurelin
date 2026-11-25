# Aurelin Roadmap

Last updated: 2026-11-24

This checklist tracks Aurelin's progress toward a cohesive Laravel 13-inspired
Rust framework. Checked items are implemented and verified at the relevant
public boundary. Unchecked items remain planned and do not imply a delivery
date or a Laravel-wide parity claim.

The immediate product target is a guided `aurelin new` experience that produces
a runnable React or Vue application with authentication, SQLite persistence,
and queue/job support. That experience gates Aurelin's initial usable release;
the broader framework capabilities remain tracked after it.

## Settled direction

- [x] Use Laravel 13 as the behavioral reference.
- [x] Prefer established Rust crates and frontend packages over rebuilding
      their underlying functionality.
- [x] Support React and Vue as the first-class Inertia frontend choices.
- [x] Keep generated-application persistence and frontend UI application-owned.
- [x] Test observable behavior through real application boundaries.
- [x] Keep Svelte, Livewire, Blade, WorkOS authentication, and teams out of
      scope.
- [x] Defer Inertia SSR to a later milestone.

## Initial usable release

The release is complete when a developer can install Aurelin, run
`aurelin new`, answer a short walkthrough, and receive a working application
without referring to the Aurelin source checkout. The generated application
must include the selected React or Vue frontend, the built-in authentication
experience, SQLite persistence, and a usable queue worker and job workflow.

### Installable CLI and published dependencies

- [x] Add crates.io package metadata to every Aurelin crate.
- [x] Give publishable Aurelin path dependencies registry version
      requirements while preserving local workspace development.
- [x] Publish the initial 0.1.0 versions of `aurelin-framework`,
      `aurelin-passkeys`, `aurelin-fortify`, and `aurelin-cli`.
- [x] Publish a coordinated release containing guided generation and queue
      support in dependency order.
- [x] Rename the CLI package to `aurelin` and publish it under the canonical
      crates.io name.
- [x] Install the released CLI with `cargo install aurelin` on a clean
      machine or equivalent isolated environment.
- [x] Generate registry dependencies by default instead of requiring
      `--framework-path`.
- [x] Retain an explicit local-path override for Aurelin framework development.
- [x] Prevent ignored development artifacts such as starter `node_modules`
      directories from being embedded in the CLI package or generated output.

### Guided application creation

- [x] Make `aurelin new <path>` start an interactive walkthrough when required
      choices were not supplied as command-line options.
- [ ] Let the walkthrough select React or Vue and confirm the application
      features included in the generated project.
- [x] Keep equivalent non-interactive options for scripts and continuous
      integration.
- [ ] Feed interactive and non-interactive choices into one typed generation
      configuration rather than maintaining separate scaffold paths.
- [x] Generate a React or Vue application with the built-in authentication
      experience.
- [x] Configure SQLite and run the authentication schema migrations for a new
      application.
- [x] Print concise next steps that are accurate for the generated choices.

### Queue and job workflow

- [x] Establish the Laravel 13 queue behavior and extension boundary to
      translate, then verify the selected Apalis capabilities against it.
- [ ] Provide framework-owned queue configuration, dispatch, worker, retry,
      and failure conventions over Apalis.
- [x] Configure a SQLite-backed queue by default in generated applications.
- [x] Provide the required queue schema through generated application
      migrations and retain exhausted jobs in the durable Apalis job store.
- [x] Give generated applications an application-owned job definition and a
      clear way to run a worker.
- [x] Verify successful dispatch and execution through a generated
      application's real queue and worker boundary.
- [x] Verify retries, exhausted jobs, failed-job operations, and continued
      processing of unrelated queued work.

### Release acceptance

- [ ] From an installed `aurelin` binary, complete the walkthrough in a clean
      environment without access to the Aurelin repository.
- [ ] Start the generated application, register and authenticate a user, and
      persist that user in SQLite.
- [ ] Dispatch a generated application job, process it with a worker, and
      observe its intended application-visible result.
- [ ] Compile, lint, test, and typecheck the generated React and Vue variants
      using only their declared registry and frontend dependencies.

## 1. Application foundation

- [x] Create Cargo applications with `aurelin new`.
- [x] Refuse to overwrite an existing destination.
- [x] Validate generated Cargo package names.
- [x] Separate process startup, bootstrap, routes, configuration, and tests.
- [x] Load `.env` and environment-specific configuration without mutating the
      process environment.
- [x] Give deployment environment variables precedence over dotenv files.
- [x] Validate typed listener configuration before binding.
- [x] Serve real HTTP through Axum and Tokio.
- [x] Provide the `/up` health endpoint.
- [x] Shut down gracefully after Ctrl+C.
- [x] Test generated applications through their compiled process and HTTP
      boundary.
- [ ] Add Laravel's health diagnostic event hook after an event dispatcher is
      available.
- [ ] Add consistent Laravel-style HTTP error rendering.

## 2. Inertia React and Vue foundation

- [x] Integrate Inertia 3 with Axum through `inertia_rs`.
- [x] Generate React 19 applications using the official Inertia React client.
- [x] Generate Vue 3 applications using the official Inertia Vue client.
- [x] Integrate Vite development and production builds.
- [x] Embed production frontend assets in generated binaries.
- [x] Wire asset versions into Inertia responses.
- [x] Verify initial HTML visits and Inertia JSON visits.
- [x] Verify script-safe page serialization.
- [x] Verify stale-version full-page reload responses.
- [x] Compile, lint, test, and typecheck generated React and Vue applications.
- [ ] Reassess async prop support against future starter requirements.
- [ ] Reassess validation error bags where `inertia_rs` remains partial.
- [ ] Add Inertia SSR in the later SSR milestone.

## 3. Built-in authentication starter

### Package and security foundations

- [x] Keep password hashing, sessions, rate limiting, reset-token primitives,
      and signed URLs in `aurelin-framework`.
- [x] Keep Fortify-equivalent workflows and contracts in `aurelin-fortify`.
- [x] Keep WebAuthn ceremonies in `aurelin-passkeys`.
- [x] Keep user persistence, migrations, actions, account-settings routes, and
      pages in the generated application; mount Fortify-owned authentication
      routes from `aurelin-fortify`.
- [x] Hash passwords with Laravel's default bcrypt cost.
- [x] Rotate session IDs after authentication.
- [x] Protect state-changing requests with CSRF validation.
- [x] Persist sessions in SQLite for generated authentication applications.

### Login and registration

- [x] Render registration and login screens in React and Vue.
- [x] Register users with normalized email addresses and validated passwords.
- [x] Authenticate users by email and password.
- [x] Prevent authenticated users from using guest-only authentication routes.
- [x] Log users out and invalidate their authenticated session.
- [x] Support remember-me sessions with Laravel's five-year lifetime.
- [x] Limit password login attempts to five per minute per normalized email and
      IP address.

### Password reset and policy

- [x] Request password-reset links without disclosing whether an account
      exists.
- [x] Store only digests of random password-reset tokens.
- [x] Expire reset tokens after 60 minutes.
- [x] Throttle repeated reset-link requests.
- [x] Consume reset tokens after one successful password change.
- [x] Apply Laravel-style local and production password rules.
- [x] Expose browser `passwordrules` hints.
- [x] Check production passwords against HIBP's padded range API without
      sending the complete password hash.
- [x] Allow password selection when the external compromised-password service
      is unavailable.

### Email verification

- [x] Send email-verification notifications after registration and on request.
- [x] Generate temporary signed verification URLs bound to the user and email.
- [x] Reject modified, expired, wrong-user, and wrong-email verification links.
- [x] Protect verified-only application and settings routes.
- [x] Redirect already-verified users according to Laravel's starter behavior.
- [x] Limit verification requests to six per minute per user.

### Password confirmation

- [x] Render the password-confirmation screen only for authenticated users.
- [x] Verify the current password before recording confirmation in the session.
- [x] Require recent password confirmation for the security screen and
      sensitive two-factor and passkey operations.

### Two-factor authentication

- [x] Generate and encrypt TOTP secrets and recovery codes.
- [x] Provide QR-code and manual setup-key access.
- [x] Require a valid TOTP code to complete setup.
- [x] Challenge two-factor-enabled users after password login.
- [x] Accept and rotate single-use recovery codes.
- [x] Reject replayed TOTP codes.
- [x] Regenerate recovery codes.
- [x] Disable two-factor authentication.
- [x] Limit two-factor login attempts to five per minute.

### Passkeys

- [x] Use `webauthn-rs` for WebAuthn ceremonies.
- [x] Require discoverable, user-verified credentials.
- [x] Use stable opaque application-specific user handles.
- [x] Keep ceremony state on the server.
- [x] Register, authenticate with, confirm through, and delete passkeys.
- [x] Persist credential-counter updates.
- [x] Use the official `@laravel/passkeys` client in React and Vue.
- [x] Expose Laravel's well-known passkey endpoints.
- [x] Limit passkey ceremonies to ten attempts per minute per credential or
      session and IP address.
- [ ] Qualify the complete flow in a real browser with a platform
      authenticator when a reliable test setup is approved.

### Account settings

- [x] Display and update profile information.
- [x] Enforce case-insensitive email uniqueness.
- [x] Reset verification when an email address changes.
- [x] Preserve verification when the email address does not change.
- [x] Update passwords after validating the current password.
- [x] Limit password updates to six attempts per minute.
- [x] Invalidate other authenticated sessions after a password change.
- [x] Delete an account after validating its current password.
- [x] Provide appearance settings in React and Vue.
- [x] Consolidate password, two-factor, and passkey management on the security
      screen.

### Mail delivery

- [x] Log outgoing mail by default in local development.
- [x] Send mail through configurable SMTP using `lettre`.
- [x] Support authenticated TLS, STARTTLS, and explicit unencrypted local SMTP.
- [x] Test SMTP delivery through a controlled server boundary.

### Authentication closeout

- [ ] Decide whether `aurelin new` should allow registration, email
      verification, two-factor authentication, passkeys, and password
      confirmation to be individually omitted. All are currently installed,
      matching Laravel's default selection.
- [ ] Implement and test feature-selective generation if it is approved.
- [x] Perform a final targeted comparison of routes, props, redirects,
      validation responses, and security invariants against the pinned Laravel
      13 starter reference.
- [ ] Map Laravel authentication events after Aurelin has an approved general
      event-dispatcher boundary.
- [x] Document every remaining auth difference found by the targeted comparison
      as an intentional adaptation,
      deferred framework capability, or unresolved gap.

## 4. General application development

### HTTP application structure

- [ ] Define general routing conventions beyond the generated authentication
      application.
- [ ] Define reusable middleware conventions and application registration.
- [ ] Expand request validation and form-feedback behavior beyond auth flows.
- [ ] Implement consistent application error responses.

### Data and persistence

- [ ] Define general SeaORM model, relationship, and query conventions.
- [ ] Support application migrations outside the starter's auth schema.
- [ ] Add database transactions and data-integrity conventions.
- [ ] Add factories and seed data workflows.
- [ ] Qualify supported database drivers beyond SQLite.

### Authorization, configuration, and tooling

- [x] Implement authorization policies and route/action authorization.
- [ ] Expand typed application and service configuration.
- [ ] Add application-development and inspection commands to the CLI.

## 5. Framework services

Each service must begin with a verified Laravel 13 contract. A selected crate
does not by itself complete a capability.

### Events

- [x] Implement event dispatch and listener registration.
- [x] Define synchronous listener ordering and failure behavior.
- [ ] Integrate queued listeners after the queue boundary exists.

### Cache

- [x] Implement process-local Moka and distributed Fred/Redis cache drivers,
      verified through a shared contract against Moka and a controlled real
      Redis server.
- [x] Match the implemented Laravel cache subset's observable expiration,
      conditional write, retrieval, removal, integer mutation, remember,
      multiple-key, prefix, and flush behavior.
- [x] Implement atomic owner-token cache locks with acquisition, restoration,
      owner-aware and forced release, refresh, expiration, and blocking
      timeouts.
- [ ] Add tags, stale-while-revalidate, memoization, events, failover, custom
      drivers, and a multiple named-store manager.

### Scheduling

- Queue and worker support is part of the initial usable release milestone
  above.
- [x] Implement explicitly registered async scheduled callbacks, single-run
      cron due evaluation, and cache-lock overlap prevention.
- [ ] Add the deferred scheduling features listed in `docs/ARCHITECTURE.md`,
      including one-server occurrence election and scheduler worker mode.

### Notifications and mail

- [x] Implement reusable notifications with per-recipient mail channel selection.
- [x] Add general application-facing mail composition and application-owned rendering conventions.
- [x] Deliver composed mail through the existing Apalis queue and real worker boundary.
- [ ] Add queued notification objects, per-channel queue options, events, localization, and additional channels.

### Filesystems and uploads

- [x] Implement qualified local and generic object-storage disks with
      `object_store`, safe relative paths, prefixes, streaming, metadata, copy,
      move, and explicit missing-object behavior.
- [x] Implement streamed temporary multipart uploads, total/per-field/schema
      limits, opaque names, encoded configured public URLs, and explicit
      unsupported visibility and signing capabilities.
- [ ] Qualify cloud-provider configuration, portable visibility or ACL support,
      MIME detection, and provider-backed temporary URLs.

### Localization and outbound HTTP

- [ ] Implement localization and translated validation/auth messages.
- [ ] Add configured outbound HTTP client conventions over `reqwest`.

## Deferred work

- Inertia SSR.
- Full browser WebAuthn qualification pending an approved reliable test setup.

## Excluded work

- Svelte and Livewire starters.
- Blade or a Blade-equivalent templating layer.
- WorkOS authentication and team-enabled starter variants.

## Validation baseline

- [x] Format workspace Rust code with `cargo fmt`.
- [x] Lint all workspace targets with warnings denied.
- [x] Run all workspace tests.
- [x] Compile, lint, and test generated backend, React, and Vue applications.
- [x] Typecheck the React and Vue frontend templates.

Use the following commands for the current baseline:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

React and Vue changes must also pass `npm run typecheck` in their respective
starter directories. Generated-application checks remain authoritative because
template source alone is not the final generated product.

## Reference documents

- `CONTEXT.md` — fixed project direction and scope.
- `docs/ARCHITECTURE.md` — implemented architecture and verified behavior.
- `docs/crate-mapping.md` — researched dependency choices and cautions.
- `README.md` — current user-facing project summary and commands.
