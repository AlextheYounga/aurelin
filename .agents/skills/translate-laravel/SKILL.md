---
name: translate-laravel
description: Translate Laravel features and first-party package boundaries into Aurelin faithfully and idiomatically. Use whenever implementing, changing, reviewing, or planning Laravel-equivalent behavior, package or crate structure, conventions, generated application code, starter-kit flows, framework APIs, validation, routing, authentication, persistence, queues, mail, events, middleware, configuration, or developer tooling. Requires inspecting the applicable Laravel documentation, package ownership, source, tests, and official starter implementation before designing Rust code, then reusing Aurelin's mapped crates rather than reinventing upstream functionality.
---

# Translate Laravel

Treat Laravel as the behavioral reference and Rust as the implementation language. Preserve Laravel's verified developer experience and observable semantics without copying PHP mechanics or surrendering Rust's type system, ownership model, concurrency, or ecosystem.

## Establish the reference first

Before designing or writing feature code:

1. Read the relevant sections of `CONTEXT.md` and identify the Laravel version Aurelin targets.
2. Locate the authoritative Laravel implementation for the requested capability. Inspect, as applicable:
   - Laravel documentation;
   - framework or first-party package source;
   - upstream tests;
   - default configuration, migrations, and generated files;
   - official Vue or React starter-kit code.
3. Distinguish framework defaults from optional packages, configurable behavior, starter-kit behavior, and application-specific examples.
4. Identify the Composer package that owns the behavior and its public integration boundary. Do not infer ownership from the starter file that calls it.

Do not implement from memory, a third-party tutorial, or a Laravel-shaped API guess. Documentation describes the contract, but inspect source and tests whenever sequence, defaults, edge cases, or extension behavior matter.

## Preserve package ownership

Translate Laravel's package ecosystem as part of the architecture, not only its runtime behavior.

1. Map each applicable upstream package to its Aurelin crate before placing code. Use `laravel/framework` as the default owner only for behavior actually supplied by Laravel Framework. Give independent first-party packages such as Fortify an equivalent focused crate.
2. Keep the same dependency direction: generated applications depend on Aurelin packages; packages must not depend on the CLI or generated starter code.
3. Keep package-provided workflows, contracts, middleware, controllers, pipelines, and defaults in the equivalent Aurelin crate. Keep application implementations of documented extension contracts, models, migrations, configuration, route selection, and frontend views in the generated application.
4. Keep the CLI limited to selecting packages, declaring dependencies, and installing application-owned templates. Never use an embedded starter directory as the primary home of reusable framework or package behavior.
5. Preserve a package boundary by default when it represents an independently versioned Laravel product or a meaningful public integration seam. Do not create one crate per PHP namespace, class, or internal component.
6. Record deliberate package-boundary deviations and the concrete Rust or dependency constraint that requires them.

For example, translate Laravel Framework hashing, sessions, rate limiting, signed URLs, and password-broker primitives into `aurelin-framework`; translate Fortify's headless authentication workflows and action contracts into an `aurelin-fortify` crate; leave the application's user model, action implementations, migrations, and React or Vue pages in the generated project.

## Extract the translation contract

Write down only the facts needed for the current capability:

- public API and developer workflow;
- default configuration and generated application structure;
- inputs, validation, authorization, and security boundaries;
- state transitions, persistence, transactions, and side effects;
- success results, errors, redirects, status codes, and user-visible messages;
- ordering, idempotency, concurrency, and failure behavior where relevant;
- extension points and deliberate customization seams;
- behavior covered by upstream tests;
- features that are optional or explicitly outside the requested scope.

Use this as the behavioral contract. Do not turn the research into a speculative design document or expand the task to adjacent Laravel features.

## Reuse before building

1. Search Aurelin for an existing implementation or convention.
2. Read `docs/crate-mapping.md` and load the `follow-crate-mapping` skill for mapped capabilities.
3. Verify the selected crate's real API, version compatibility, features, maintenance, license, and behavior from authoritative documentation or source.
4. Identify the exact gap between the verified Laravel contract and what Rust, Aurelin, and the selected crate already provide.
5. Write only the integration, adaptation, conventions, or starter code needed to close that gap.

Treat crate reuse as an architectural requirement, including inside Aurelin equivalents of Laravel packages:

- Build Aurelin packages by composing the leading crates in `docs/crate-mapping.md`; do not interpret package parity as permission to reimplement their subsystems.
- Search crates.io, docs.rs, the local dependency graph, and the mapping before writing foundational machinery.
- Put Laravel-compatible policy, defaults, orchestration, and adaptation around the selected crate while leaving the crate responsible for its established primitive.
- Add custom machinery only for a verified behavioral gap. State the gap and why the mapped crate cannot satisfy it.
- If a new crate is a better fit than the mapped candidate, follow the crate-mapping deviation process and update the mapping rather than silently bypassing it.
- Do not create an Aurelin wrapper that merely renames a clear Rust API. An Aurelin package should add Laravel behavior, package integration, conventions, or a meaningful extension boundary.

Do not recreate an ORM, validator, router, container, event system, queue, mailer, session layer, asset pipeline, or other subsystem when the mapped stack already provides it. Do not wrap a crate merely to make its API look more like PHP when direct use is clear and coherent.

If the mapped stack cannot satisfy required behavior, demonstrate the concrete gap before adding custom machinery. Follow the crate-mapping deviation process rather than silently choosing another dependency or implementing a replacement.

## Translate behavior, not PHP structure

Preserve the meaning of Laravel's behavior while choosing idiomatic Rust boundaries.

- Use explicit types and enums instead of stringly typed states.
- Use `Result` and structured errors instead of exceptions where appropriate.
- Use ownership and borrowing instead of reproducing mutable global or request-container behavior.
- Use traits only for real extension boundaries, not to imitate facades or service-container lookup.
- Use framework extractors, middleware, ORM transactions, async tasks, and dependency injection through parameters or constructors when they naturally express the behavior.
- Preserve useful Rust guarantees such as compile-time validation, `Send`/`Sync` constraints, exhaustive matching, and safe concurrency.
- Keep generated applications recognizable to Laravel developers, but organize modules and files around clear Rust responsibilities.

Do not mechanically port classes, facades, macros, traits, service providers, active-record internals, or dynamic runtime patterns. Conversely, do not use “idiomatic Rust” as a reason to omit Laravel defaults, protections, extension behavior, or failure semantics.

## Implement the smallest faithful slice

- Keep entry points thin: extract and validate input, authorize, delegate, and return the translated response.
- Name concepts using Laravel terminology when the concept remains the same; use Rust terminology where the implementation mechanism differs.
- Preserve default behavior before adding customization.
- Keep optional Laravel features optional.
- Explain unavoidable differences caused by Rust, the selected crates, or an explicit Aurelin decision.
- Remove superseded implementations and compatibility shims once the translation replaces them.

Stop and report the gap rather than claiming equivalence when a required behavior cannot yet be implemented faithfully.

## Verify against the reference

Derive focused tests from Laravel's observable scenarios and upstream tests. Cover the relevant successful flow and critical failure or invariant boundaries through Aurelin's real public boundary.

Do not port assertions about Laravel internals or duplicate tests belonging entirely to an upstream Rust crate. Test Aurelin's translation, integration, defaults, generated output, and user-visible workflow.

Before completion:

1. Compare the finished behavior with the extracted Laravel contract.
2. Run formatting, linting, relevant tests, and generated-application checks.
3. Review for invented behavior, missing defaults, unnecessary wrappers, duplicated dependency functionality, and stale code or documentation.
4. State verified behavior, intentional adaptations, and remaining gaps.

Call a feature Laravel-equivalent only when its applicable contract is implemented and tested. Report partial translations precisely.
