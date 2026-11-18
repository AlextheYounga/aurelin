# Architecture

## Repository layout

Kiln is a Rust framework ecosystem modeled on Laravel. The repository keeps the
Rust crates together under `crates/`, so each independently usable package can
be published and consumed as a crate.

```text
kiln/
├── Cargo.toml                 # Cargo workspace root
├── CONTEXT.md                 # Product direction and fixed decisions
├── docs/
│   └── ARCHITECTURE.md        # Repository and crate organization
├── crates/
│   ├── kiln-framework/        # The Kiln framework crate
│   └── kiln-cli/              # CLI for creating and managing applications
│       └── starters/          # Application starter templates
└── laravel -> ...             # Local Laravel framework reference symlink
```

The root `Cargo.toml` defines the workspace. The Laravel symlink is reference
material only; it is not a Rust workspace member or a Kiln dependency.

## Crates

### `kiln-framework`

The framework crate provides Kiln's application-facing framework integration
and conventions. Its modules and dependencies will be introduced as specific
capabilities are implemented; the repository will not predefine a crate for
every Laravel subsystem.

### `kiln-cli`

The CLI package builds the `kiln` command used to create and manage Kiln
applications. Application starter templates belong with this CLI because it
is responsible for materializing a starter into a new project. Vue and React
starters will be added when that capability is implemented.

## Growth rule

Add crates and starter content when a concrete capability requires them. Keep
the workspace organized under `crates/` without speculatively mapping Laravel's
subsystems or creating placeholder templates. For v1, prefer integrating
existing Rust crates and frontend packages over reimplementing their
functionality.

## Rust quality checks

Workspace crates inherit the Clippy lint policy from the root `Cargo.toml`.
Clippy thresholds are in `clippy.toml`, and formatting is configured in
`rustfmt.toml`. Run the checks from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```
