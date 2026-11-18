# Kiln — Project Context

## Purpose

Kiln is a Rust web framework ecosystem modeled on Laravel. Its goal is to give developers Laravel's cohesive application-building experience through existing Rust crates, consistent conventions, and straightforward integration.

**For v1, never reinvent the wheel.** Scout for existing implementations of the required Laravel-equivalent capabilities and assemble them into a coherent framework. Kiln's contribution is the integration, configuration, conventions, and starter experience.

This document establishes direction. It does not select crates, define a feature-to-crate mapping, or claim that any functionality is already implemented.

## Fixed decisions

- **Name:** Kiln.
- **Backend:** Rust.
- **Behavioral reference:** Laravel 13.
- **Frontend:** First-class Inertia-style support.
- **Initial frontend choices:** Vue or React, using Vite.
- **Templating:** No Blade equivalent.
- **Authentication:** Working boilerplate equivalent to Laravel 13's built-in authentication starter experience for Vue and React.
- **V1 approach:** Reuse existing crates and frontend packages; write the minimum integration code necessary.

Other stack choices remain undecided. Do not treat candidates discussed elsewhere as selected dependencies.

## Follow Laravel's logic

Use Laravel's documented behavior, conventions, and developer workflow as the starting point for each capability. We are translating an established framework experience into Rust.

Before implementing a capability, inspect the relevant Laravel 13 documentation and, where needed, its source, tests, and official starter kits. Record the version or source revision used with the implementation decision so the reference is reproducible. Do not combine behavior from different Laravel versions without identifying the difference.

Preserve the meaning of the relevant behavior, including its defaults, successful flows, validation, errors, and redirects. Use idiomatic Rust to express that behavior. Matching Laravel does not require reproducing PHP's runtime mechanics or copying its internal architecture line for line.

The explicit Kiln decisions in this document take precedence where they differ from Laravel. A concrete language or integration constraint may require an adaptation; explain that specific difference instead of silently changing the behavior.

Laravel equivalence is the project's direction, not an instruction to implement the entire Laravel ecosystem at once. Work on the capability currently requested.

## Reuse before implementation

When implementation begins, investigate existing crates for the capability at hand. Do not create a broad crate mapping as part of this context document.

Check authoritative crate documentation and source for actual capabilities, supported versions, integration requirements, maintenance, and license compatibility. A package name, README claim, or previous agent answer is not proof that a proposed integration works.

Use existing libraries for the underlying functionality. Kiln may provide configuration, adapters, starter templates, and a consistent public interface where those are necessary to make the selected libraries work together. Prefer using an existing API directly when wrapping it adds no concrete value.

Keep integration code focused on the actual gap between the chosen components and the required Laravel behavior. Do not rebuild a subsystem to obtain a preferred API shape, add a general abstraction for hypothetical replacements, or duplicate functionality already supplied by a dependency.

If available libraries cannot satisfy a requirement, describe the verified gap and its effect. Do not silently replace the reuse strategy with a custom subsystem or present partial support as equivalent functionality.

## Frontend model

Inertia-style application development is a first-class framework path. Rust handles server routes and application behavior; Vue or React renders pages through the integration. Vite provides frontend development and asset builds.

Use Inertia's documented protocol and existing frontend adapters as the reference for compatibility. Reuse the existing client ecosystem rather than inventing a similar-looking protocol or a custom frontend framework. Verify version compatibility when choosing the Rust integration.

Both Vue and React starters should exercise the same backend conventions and behavior. An application chooses one frontend; it should not require both.

Provide the minimal HTML document needed to mount the frontend and load its assets. This does not imply a Blade replacement or a general-purpose server-side template system.

## Authentication starter

The target is a complete, usable authentication starter, including the backend integration and the corresponding Vue or React screens and flows.

Use the **built-in authentication variant of Laravel 13's official Vue and React starter kits** as the baseline. Determine the actual feature set and defaults from the reference before implementing them. Preserve the distinction between behavior included by default, configurable options, and separate optional integrations.

Do not reduce the requirement to a login endpoint, and do not expand it into an independently designed identity platform. Match the verified starter behavior and configuration through existing libraries.

Reference: [Laravel 13 starter kits](https://laravel.com/framework/docs/13.x/starter-kits) and [Laravel 13 authentication](https://laravel.com/framework/docs/13.x/authentication).

## Rules for agents

- **Do not invent requirements.** A feature must trace to the current task, an explicit Kiln decision, or the relevant verified Laravel behavior. An integration adjustment must trace to an actual dependency or language constraint.
- **Do not invent facts.** Verify crate names, APIs, versions, capabilities, and compatibility before relying on them. Clearly distinguish a proposal, a verified fact, and implemented behavior.
- **Do not invent concerns or guards.** Do not add restrictions, validation, fallback behavior, retries, defensive branches, or infrastructure merely because they might be useful. Preserve protections required by the referenced Laravel behavior, documented dependency requirements, or a demonstrated issue in the integration. Tie each addition to concrete evidence.
- **Keep unknowns explicit.** If a reference cannot be verified, state what remains unknown. Do not fill the gap with remembered behavior or plausible code.
- **Keep the design proportional.** Add abstractions and configuration only when they solve a present integration or application need. Avoid speculative extension points and architectural layers.
- **Test the translation and integration.** Verify observable behavior against the chosen reference, including relevant failure paths. Concentrate tests on Kiln's wiring and the starter experience; avoid duplicating upstream library tests or testing hypothetical features.
- **Report what is actually complete.** Separate working, verified behavior from research, planned support, and unresolved gaps. A dependency selection alone does not establish feature parity.

For each requested capability: establish the Laravel behavior, inspect reusable implementations, choose the smallest compatible integration, and verify the result. Keep supporting notes close to the work; this does not require a separate planning bureaucracy.

## Success criterion

A developer familiar with Laravel should recognize the application workflow, choose Vue or React, and start from working authentication without assembling the framework themselves. Existing libraries perform the underlying work; Kiln makes them operate as one coherent Laravel-style stack in Rust.

Additional reference: [Inertia](https://inertiajs.com/). Verify the applicable documentation and versions when implementation begins.

