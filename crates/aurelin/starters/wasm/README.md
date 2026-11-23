# Aurelin Leptos application

Install the stable WebAssembly target and `cargo-leptos`, then run:

```sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos --locked
cargo aurelin migrate
cargo leptos watch
```

Open <http://127.0.0.1:8000>. Leptos renders each route on the server and
hydrates the returned HTML in the browser. Application routes and components
live in `src/app.rs`; Axum-only routes and middleware remain under
`src/routes/` and `src/bootstrap.rs`.

The generated registration, login, and logout forms use typed Leptos server
functions protected by Aurelin synchronizer tokens. Password policy, user
creation, verification notification, credential validation, throttling,
session transitions, remember-me behavior, and two-factor branching delegate
to Aurelin Fortify.

Create a production server binary and browser bundle with:

```sh
cargo leptos build --release
```

No Node.js, npm, Vite, or Inertia installation is used by this frontend.
