---
name: rust-test-best-practices
description: Enforce this project's Rust test organization and quality standards. Use whenever writing, moving, reviewing, or refactoring Rust tests, test helpers, fixtures, generated Rust applications, or production modules that currently contain #[cfg(test)] or inline test modules.
---

# Rust Test Best Practices

## Keep tests outside production source

- Place all Rust test code in the owning crate's crate-root `tests/` directory.
- Never place `tests.rs`, test functions, assertions, fixtures, `#[cfg(test)]`, or `mod tests` under `src/`.
- Keep production files free of test-only imports, visibility changes, feature gates, helpers, and seams.
- Put shared integration-test setup under `tests/common/` or another focused support module below `tests/`.
- Put fixtures under `tests/fixtures/` and keep them focused on observable behavior.
- Remember that only Rust files directly inside `tests/` become Cargo integration-test targets automatically. Use subdirectories only as modules imported by those targets.
- Do not confuse a crate-root `tests/` directory with a workspace test package.

For generated applications, emit the same structure into the generated project:

```text
src/
├── lib.rs
├── main.rs
└── ... production modules
tests/
├── application.rs
├── authentication.rs
└── support/
```

## Test through a real boundary

- Test library behavior through deliberate public APIs.
- Test command behavior by executing the compiled binary through Cargo's `CARGO_BIN_EXE_<name>` environment variable.
- Observable command behavior includes accepted and rejected arguments, exit status, stdout, stderr, filesystem effects, and external-system effects.
- Give binary packages a library target for reusable parsing, orchestration, and domain behavior. Keep `main.rs` thin so the binary and integration tests use the same production boundary.
- Do not use `#[path = "../src/..."]` to compile production modules into an integration test.
- Do not reproduce production algorithms in tests.
- Do not make private items public solely for test access. Extract a cohesive public API only when callers can meaningfully use it.
- Do not add a test-support feature or workspace test crate when Cargo's integration-test boundary is sufficient.
- Test observable responses, persisted state, errors, and security invariants rather than private functions or struct layout.
- Treat an untestable private detail as a prompt to redesign a real boundary or remove an implementation-coupled assertion—not as permission to add a test seam.

## Isolate command tests

- Use a temporary working directory.
- Provide isolated `HOME` and relevant XDG directories.
- Create explicit repositories, remotes, databases, and external state when the scenario needs them.
- Capture and assert status, output, and durable effects.
- Never mutate the integration-test process's current directory or global environment.
- Keep shared command setup limited to behavior reused by multiple test targets.

## Organize by behavior

- Name test files after capabilities such as `configuration.rs`, `authentication.rs`, or `http_lifecycle.rs`.
- Use descriptive scenario-action-result test names.
- Keep setup relevant to the scenario visible. Move repeated mechanics—not important facts—into `tests/support/`.
- Keep tests repeatable and independent by isolating external state.
- Use the project's existing test framework and avoid introducing a second test style without a concrete need.

## Validate placement and behavior

Before finishing Rust work, run:

```sh
rg '#\[cfg\(test\)\]|mod tests' --glob 'src/**/*.rs'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The `rg` command must return no matches. Also inspect generated project templates and run their tests, because template source is production source for the generated application.
