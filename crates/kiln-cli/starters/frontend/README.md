# Kiln Inertia application

Install frontend dependencies once, then start the backend. The backend starts
and proxies Vite so both sides are available at <http://127.0.0.1:8000>.

```sh
npm install
cargo run
```

Run the backend and frontend checks with:

```sh
cargo test
npm run typecheck
npm run build
```

For a production binary, `vite-rs` runs the Vite build and embeds its output
while Cargo compiles the release executable:

```sh
cargo build --release
```

Routes live in `src/routes/`, application composition in `src/bootstrap.rs`,
and pages in `resources/js/pages`. SeaORM and SeaQuery migrations provide the
SQLite data layer. The starter includes registration, login, logout,
password-reset token handling, email verification, password confirmation,
persisted sessions, CSRF protection, and login throttling. It does not yet
include production mail delivery or confirmed-route enforcement.
