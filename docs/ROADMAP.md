# Aurelin Roadmap

Last updated: 2026-11-19

This checklist tracks Aurelin's progress toward a cohesive Laravel 13-inspired
Rust framework. Checked items are implemented and verified at the relevant
public boundary. Unchecked items remain planned and do not imply a delivery
date or a Laravel-wide parity claim.

This roadmap tracks framework and application-development capabilities.

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

- [ ] Implement authorization policies and route/action authorization.
- [ ] Expand typed application and service configuration.
- [ ] Add application-development and inspection commands to the CLI.

## 5. Framework services

Each service must begin with a verified Laravel 13 contract. A selected crate
does not by itself complete a capability.

### Events

- [ ] Implement event dispatch and listener registration.
- [ ] Define synchronous listener ordering and failure behavior.
- [ ] Integrate queued listeners after the queue boundary exists.

### Cache

- [ ] Implement process-local and distributed cache drivers.
- [ ] Match Laravel's observable cache expiration and mutation behavior.
- [ ] Implement atomic cache locks and required Laravel cache semantics.

### Queues and scheduling

- [ ] Implement queues, retries, failed jobs, and workers with `apalis`.
- [ ] Implement scheduled tasks and overlap prevention.

### Notifications and mail

- [ ] Implement reusable notifications and channel selection.
- [ ] Add general application-facing mail composition and template conventions.
- [ ] Integrate queued mail after the queue boundary exists.

### Filesystems and uploads

- [ ] Implement local and object-storage disks with `object_store`.
- [ ] Implement safe uploads, URLs, visibility, and streaming limits.

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
