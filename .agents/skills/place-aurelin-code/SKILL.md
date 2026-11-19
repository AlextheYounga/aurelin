---
name: place-aurelin-code
description: Determine and enforce the correct Aurelin package, framework crate, CLI, generated-application, and starter-template boundary before code is written. Use whenever implementing, modifying, moving, reviewing, or planning Aurelin behavior; adding routes, handlers, middleware, workflows, policies, defaults, configuration, persistence, frontend code, scaffolding, or templates; translating Laravel or first-party package behavior; or touching anything under crates/aurelin-cli/starters. Requires proving upstream ownership, recognizing that starter files are copied into every generated project, and blocking reusable framework or package logic from being placed in templates.
---

# Place Aurelin Code

Decide ownership before implementation. A working generated application is not
evidence that behavior is in the right package.

## Perform the placement gate before editing

For each capability, establish these four facts and state them briefly before
writing code:

1. **Upstream owner** — identify the Composer package and source path that
   supplies the behavior in Laravel. Trace the invoked controller, middleware,
   workflow, contract, or service to its definition; do not infer ownership
   from the starter file that calls it.
2. **Aurelin owner** — name the existing or required Aurelin crate that
   corresponds to that package or cross-cutting framework capability.
3. **Application seam** — identify only the model, action implementation,
   configuration, route selection, view, or other documented extension point
   the generated application owns.
4. **Generation effect** — inspect the scaffolder and determine whether the
   proposed file is embedded, copied, transformed, or referenced. Treat every
   copied file as code permanently forked into each new application.

Use this decision shape:

```text
Capability:
Laravel owner:
Aurelin owner:
Generated application owns:
```

If upstream ownership or the Aurelin destination is unresolved, stop and ask
before implementation. Do not use a starter directory as the temporary answer.

## Apply the ownership rules

### `aurelin-framework`

Place behavior here when Laravel Framework owns a reusable, cross-cutting
primitive or integration, such as generic HTTP behavior, sessions, hashing,
encryption, signed URLs, rate-limit mechanics, configuration, or mail transport.

Do not put independent first-party package behavior here merely because this
crate is named “framework.”

### Focused package crates

Place independently packaged Laravel behavior in its Aurelin equivalent:

- Fortify workflows, handlers, middleware, route contracts, defaults, and
  extension contracts belong in `aurelin-fortify`.
- WebAuthn ceremony mechanics belong in `aurelin-passkeys`.
- Future independently versioned Laravel products should retain a focused crate
  boundary when that boundary is meaningful in Rust.

Package code may accept application implementations through traits, typed
configuration, constructors, or handler state. It must not depend on the CLI or
generated starter code.

### Generated applications and starter templates

Starter templates may contain only application-owned output:

- models, entities, and application database migrations;
- implementations of documented package extension contracts;
- application configuration and package selection;
- application routes and thin package route mounting;
- application-specific controllers and policies;
- React or Vue pages, components, and application-owned presentation;
- thin bootstrap composition;
- tests proving the generated application works through real boundaries.

A starter file is not framework source. Every reusable workflow placed there is
duplicated into every generated project and no longer receives package fixes.

### CLI

The CLI may select packages, collect generation choices, declare dependencies,
copy application-owned templates, and perform explicit generation transforms.
It must not be the runtime home of framework or package behavior.

## Enforce hard prohibitions

Never place the following in a starter to make an end-to-end scenario pass:

- reusable authentication, authorization, queue, mail, cache, storage, event,
  validation, or protocol workflows;
- package-owned controllers, route handlers, middleware, pipelines, policies,
  defaults, or security semantics;
- a duplicate implementation of behavior already present in an Aurelin crate;
- logic expected to receive framework bug fixes across existing applications;
- an abstraction whose only reason for living in the application is that the
  package API has not been designed yet.

Do not approve placement because code is small, has one caller, is easiest to
reach from a template, or resembles code visible in an official starter. Trace
ownership first.

Do not create a “temporary vertical slice” in a starter unless the user
explicitly approves the architectural debt, its removal boundary, and why the
correct package cannot be changed first.

## Extend packages before templates

When a generated application needs reusable behavior that an Aurelin package
cannot yet expose:

1. Add or improve the package-level public seam.
2. Test package behavior through that public seam.
3. Add the smallest application-owned adapter or configuration in the starter.
4. Test the generated application integration.

Do not design a generic trait for hypothetical implementations. Introduce only
the extension seam Laravel exposes or current generated applications require.

## Review changes for placement failures

Before completion, inspect every changed file and answer:

- Would two independently generated applications receive identical behavioral
  logic from this file? If yes, it probably belongs in a package.
- Would a security or correctness fix need to reach existing applications? If
  yes, prefer package code over copied template code.
- Is the starter implementing a controller, middleware, or workflow owned by a
  Laravel package? If yes, move it to the equivalent Aurelin crate.
- Does the generated file only configure, implement an extension contract,
  select routes, or render application UI? If yes, starter placement may be
  correct.
- Does dependency direction remain generated app → package → lower-level
  package/framework, never the reverse?

List intentional starter logic in the completion report. Treat unexplained
reusable logic under `crates/aurelin-cli/starters` as a failed review.

## Correct existing violations

When existing starter code violates these rules:

1. Stop adding behavior to the misplaced implementation.
2. Preserve observable behavior with existing generated-application tests.
3. Extract reusable behavior to the correct crate.
4. Replace copied logic with thin application configuration or adapters.
5. Delete the superseded starter implementation.
6. Run package tests and generated-application tests.

Do not call a package translation complete while its reusable runtime workflow
still lives primarily in an embedded starter.
