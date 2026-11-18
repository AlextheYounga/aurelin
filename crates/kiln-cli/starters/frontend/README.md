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

For a production binary, build the frontend before compiling in release mode;
the Vite output is embedded in the executable:

```sh
npm run build
cargo build --release
```

Routes live in `src/routes/`, application composition in `src/bootstrap.rs`,
and pages in `resources/js/pages`. The starter includes SQLite-backed
registration, login, logout, password-reset token handling, persisted sessions,
CSRF protection, and login throttling. It does not yet include email
verification, password confirmation, or production mail delivery.
