# Aurelin application

```sh
cargo aurelin serve
```

Run `cargo aurelin` to list every command provided by the application.
Application migrations live in `database/migrations/`; run them with `cargo
aurelin migrate`.
Add application-specific commands in `src/console/mod.rs` and delegate their
business work to focused application modules.

Open <http://127.0.0.1:8000>. Run commands from this directory; `.env` is
resolved relative to the current working directory. External environment
variables override `.env`. Set `SERVER_HOST` and `SERVER_PORT` to change the
listener, and press Ctrl+C to shut down gracefully.

Define HTTP routes in `src/routes/http.rs`. Compose routes, Axum state, and
middleware in `src/bootstrap.rs`. Run `cargo test` to exercise your app.

Define background jobs in `src/jobs.rs`, dispatch them through the SQLite queue,
and run a worker in another terminal:

```sh
cargo aurelin queue:work
```

This is the backend foundation, not the finished Vue/React authentication
starter. Sessions, CSRF, database access, frontend assets, and authentication
have not been installed. HTTP routes do not yet have Laravel's `web` middleware.
