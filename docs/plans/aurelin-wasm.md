# Native full-stack WASM applications

## Intent

Aurelin supports React and Vue applications through Inertia, but it does not
offer a browser application written in Rust. Treating Rust/WASM as another
Inertia renderer would duplicate routing, data-loading, mutation, and
hydration behavior already supplied by a full-stack Rust web framework.

A developer can select a WASM frontend when creating an Aurelin application
and receive a runnable full-stack Leptos application. The application renders
meaningful HTML on the server, hydrates that HTML with WebAssembly, navigates
with Leptos routing, and communicates with the native Aurelin process through
typed Leptos server functions and resources. It retains Aurelin's existing
authentication behavior and security boundaries without adding an Inertia
client or a JavaScript application toolchain.

## Definitions

**WASM frontend:** The `wasm` application-generation choice. It currently
means a Leptos application compiled to WebAssembly for browser hydration, not
an abstraction over arbitrary Rust UI frameworks and not a backend compiled to
WASI.

**Full-stack Leptos:** One application UI compiled for two targets: native Rust
with Leptos server-side rendering and browser WebAssembly with Leptos
hydration. Leptos routing, resources, actions, and server functions form the
server/browser application boundary.

**Server function:** A public HTTP transport generated from a typed Rust
function. It remains subject to authentication, authorization, validation,
CSRF protection, rate limiting, and safe error handling like any other public
endpoint.

**Hydration:** Attaching browser-side reactive state and event handlers to the
HTML already rendered by the server, rather than replacing the initial page
with an empty client-rendered shell.

## Scope

The WASM frontend includes:

- an `aurelin new` frontend choice named `wasm`;
- Leptos server-side rendering, browser hydration, routing, resources,
  actions, and server functions;
- the built-in Aurelin authentication and account-settings experience,
  including the existing registration, password, email-verification,
  two-factor, and passkey workflows;
- integration with the existing Axum process, Aurelin application state,
  sessions, CSRF protection, Fortify workflows, and passkey ceremonies;
- a Rust-native build and development workflow coordinated by
  `cargo-leptos`; and
- generated-application validation through the same real product boundaries
  used for the existing frontend starters.

The following constraints are fixed:

- reusable authentication and security behavior remains in
  `aurelin-framework`, `aurelin-fortify`, and `aurelin-passkeys`;
- WASM presentation adapters delegate to those packages rather than copying
  their workflows into the generated application;
- UI routes and components are application-owned, while reusable Aurelin and
  Leptos view integration belongs in `aurelin-framework` and adapted workflows
  remain in their owning packages;
- the native server remains Axum-based and uses Aurelin's selected session,
  persistence, validation, and security stack; and
- existing React, Vue, and backend-only generation remains supported with its
  current public behavior.

The WASM frontend does not use Inertia page objects, an Inertia client,
Inertia component registries, Vite, npm, or Node.js. It does not add a general
REST or GraphQL API, compile the Aurelin backend to WASI, introduce another
WASM UI framework, add SSR to the React or Vue starters, or replace the
existing React and Vue Inertia convention.

## Rules

### Renders on the server and hydrates in the browser

The first visit to a WASM application returns meaningful route-specific HTML.
The browser then hydrates that document and uses Leptos routing and reactive
state for subsequent interaction. The application must not require an Inertia
page payload or discard the server-rendered document to boot a client-only
shell.

Examples:

- A guest requesting `/login` receives the login form in the HTTP response
  before browser WebAssembly executes.
- After hydration, following an application link changes the rendered route
  without a full document reload.

### Uses typed Leptos server boundaries

UI data loading and mutations cross the server/browser boundary through
Leptos resources, actions, and typed server functions. Server functions are
thin transport entry points that validate and authorize requests before
delegating application or package behavior.

Examples:

- A profile page loads its authenticated-user projection through a typed
  resource rather than an Inertia props object.
- A login server function delegates credential validation, throttling,
  password rehashing, and session transitions to `aurelin-fortify`.

### Preserves authentication protections

Choosing the WASM frontend does not weaken or fork Aurelin's authentication
semantics. Guest and authenticated route boundaries, session rotation, CSRF
protection, throttling, password policy, email verification, two-factor
authentication, and passkey ceremonies continue to be enforced by their
existing owning packages.

Examples:

- An unauthenticated request cannot obtain protected dashboard data by
  calling its server function directly.
- A cross-site state-changing server-function request is rejected even though
  it does not pass through an Inertia form.

### Builds without a JavaScript application toolchain

A generated WASM application can be developed, checked, tested, and built
with the declared Rust and `cargo-leptos` workflow. It does not require npm,
Node.js, Vite, or JavaScript frontend packages.

Examples:

- A clean environment with the declared Rust target and `cargo-leptos`, but no
  Node.js installation, can produce the server binary and browser assets.
- Generated WASM source uses Cargo dependencies rather than a hidden
  `package.json` installation step.

### Preserves existing application choices

Adding the WASM frontend does not change the generated behavior or toolchain
of React, Vue, or backend-only applications.

Examples:

- `aurelin new --frontend react` still produces the existing React/Inertia
  application.
- Omitting `--frontend` still produces the backend-only application.

## Core scenarios

### Renders and hydrates the generated WASM application

Rule: #renders-on-the-server-and-hydrates-in-the-browser
Requirement: A generated WASM application provides server-rendered initial UI and hydrated client-side navigation.
Description: This is the primary user-visible capability of the new frontend choice.
Experiment: Generate a WASM application through the installed CLI boundary, build it with its declared workflow, run its real server, and open its login route in a controlled browser. Inspect the initial HTTP document before WebAssembly executes, then allow hydration and follow an application link while observing the browser document lifecycle.
Expected: The initial document already contains the route-specific login UI; hydration completes without replacing it with an empty shell; and the application link renders its destination without a full document reload.
Rejects: A client-only WASM shell, an Inertia page bootstrap disguised as Leptos, or navigation that always reloads the complete document.

### Completes authentication through typed WASM interactions

Rule: #uses-typed-leptos-server-boundaries
Requirement: A user can complete the starter's principal authentication journey through the generated Leptos UI and typed server boundary.
Description: Existing package-level tests continue to prove detailed authentication semantics; this scenario proves that the WASM presentation reaches those real workflows.
Experiment: Run a generated and migrated WASM application against isolated persistence, use the hydrated UI to register a user, complete the controlled email-verification path, visit the protected dashboard, sign out, and sign back in with the registered credentials.
Expected: Each action returns the corresponding authenticated UI state, persisted user and session state are observed through the real application boundary, the protected dashboard is available only while authenticated, and no Inertia request or page payload participates.
Rejects: Static auth screens, a fake frontend-only login state, a parallel WASM authentication implementation, or a transport that cannot complete the existing Fortify workflow.

### Enforces security on direct server-function requests

Rule: #preserves-authentication-protections
Requirement: Typed server functions do not provide a route around Aurelin's authentication and request protections.
Description: Browser UI checks are not a security boundary, so the public transport is exercised directly.
Experiment: Against the generated application's real HTTP boundary, call a protected data function without an authenticated session and submit a state-changing function from a cross-site request context without valid CSRF proof; then perform the corresponding calls through an authenticated same-site session.
Expected: The unauthenticated and cross-site requests are rejected without protected data or mutation, while the authenticated same-site requests are accepted through the same package-owned authorization and CSRF rules.
Rejects: Authorization implemented only in Leptos components, server functions mounted outside the session or CSRF middleware, or duplicated weaker security rules.

### Builds the generated WASM application without Node.js

Rule: #builds-without-a-javascript-application-toolchain
Requirement: The generated WASM application has a complete Rust-native build path.
Experiment: Generate the WASM variant in an isolated directory and run its documented formatting, linting, test, and production-build commands with a controlled executable path that provides the declared Rust tooling and `cargo-leptos` but no `node`, `npm`, or `vite` executables.
Expected: All commands succeed and produce a runnable native server plus its browser WebAssembly assets without installing or invoking a JavaScript package.
Rejects: A nominally Rust-native starter that contains a hidden npm prebuild, invokes Vite transitively, or cannot produce deployable browser assets.

### Retains the existing generated application variants

Rule: #preserves-existing-application-choices
Requirement: Introducing WASM leaves the established React, Vue, and backend-only starter outcomes intact.
Experiment: Generate each existing variant through the CLI and run its established generated-application validation, including its existing frontend toolchain where applicable.
Expected: The React and Vue variants still compile and exercise their Inertia boundaries, the backend-only variant still builds and serves its HTTP routes, and their documented generation commands remain accepted.
Rejects: A shared scaffold or command change that silently moves existing variants onto Leptos, removes their dependencies, or breaks their generated applications.
