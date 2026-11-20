# Aurelin Inertia application

Install frontend dependencies once, then start the backend. The backend starts
and proxies Vite so both sides are available at <http://127.0.0.1:8000>.

```sh
npm install
cargo aurelin migrate
cargo aurelin dev
```

Run `cargo aurelin` to list every command provided by the application. Use
`cargo aurelin serve` to start only the HTTP server without Vite.
Add application commands in `src/console/mod.rs`; keep their handlers thin and
delegate business work to the actions in `src/actions/`.

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

Routes live in `src/routes/`, configuration in `src/config/`, application
composition in `src/bootstrap.rs`, database migrations in
`database/migrations/`, and pages in `resources/js/pages`. SeaORM and SeaQuery
migrations provide the SQLite data layer. The starter includes registration,
login, logout,
remember-me sessions, password-reset token handling, email verification,
password confirmation, profile and security settings, two-factor
authentication, passkeys, CSRF protection, and throttling.

Mail is logged locally by default. For production delivery, set
`MAIL_MAILER=smtp` and configure `MAIL_HOST`, `MAIL_PORT`, `MAIL_ENCRYPTION`,
`MAIL_FROM_ADDRESS`, and optional SMTP credentials in `.env`.

Define background jobs in `src/jobs.rs`. The application uses the same SQLite
database for its default durable queue. Run a worker in another terminal with:

```sh
cargo aurelin queue:work
```
