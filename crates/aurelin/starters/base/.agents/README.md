# Aurelin agent resources

This directory gives coding agents project-local Aurelin guidance without
requiring Aurelin Assist or a global agent configuration.

Available skills:

- `aurelin-development`: routes, handlers, middleware, configuration, and
  application structure.
- `aurelin-data-and-jobs`: SeaORM data access, migrations, queues, and schedules.
- `aurelin-testing-and-debugging`: focused tests, generated-project checks, and
  local diagnostics.

The shared framework reference is in `docs/aurelin.md`. The project `README.md`,
`Cargo.toml`, and source tree remain authoritative because generated projects
vary by selected frontend and optional features.
