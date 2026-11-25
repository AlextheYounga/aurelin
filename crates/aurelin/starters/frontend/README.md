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
Agent-focused Aurelin skills and framework guidance live in `.agents/`.
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

### Inertia SSR

SSR requires Node.js 22 or newer in production as well as development. Install
production npm dependencies on the renderer host: the SSR bundle imports the
official Inertia and React or Vue packages at runtime.

```sh
npm ci
npm run build:all
npm run start:ssr
```

Run the renderer as a separately supervised process alongside the Rust server;
the Rust request path never starts Node. It listens on `127.0.0.1:13714` by
default. With `curl` installed, `npm run health:ssr` checks readiness and `npm run stop:ssr` requests
shutdown. Restrict network access to this service: the official renderer also
exposes a shutdown endpoint. For deployments, build both bundles from the same
revision, restart the renderer and Rust process together, check health, and
roll both back together if verification fails. Ensure the SSR bundle and its
production `node_modules` are deployed to the renderer process.

For local development, run `npm run build:ssr`, start `npm run start:ssr` in
another terminal, then use `cargo aurelin dev`; rebuild and restart the renderer
after frontend changes. Vite serves the browser assets separately.

The backend integration is configured with
`INERTIA_SSR_ENABLED` (default `false`), `INERTIA_SSR_URL` (default
`http://127.0.0.1:13714/render`), `INERTIA_SSR_TIMEOUT_MS` (default `2000`),
and `INERTIA_SSR_STRICT` (default `false`). The URL must point to a trusted,
independently managed Inertia renderer; the backend sends page props to it.
Rendering errors are logged and fall back to client rendering unless strict
mode is enabled. Set `INERTIA_SSR_ENABLED=true` when the renderer is available.
For the real Rust-to-Node integration test, run `npm run build:ssr` followed by
`cargo test --test real_inertia_ssr -- --ignored`; it requires a free port 13714.

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

Jobs retry up to the attempt limit selected when they are dispatched. Inspect
and operate exhausted jobs with `cargo aurelin queue:failed`,
`cargo aurelin queue:retry <id>`, `cargo aurelin queue:forget <id>`, and
`cargo aurelin queue:flush`. Queue schema is installed only by `cargo aurelin
migrate`.
