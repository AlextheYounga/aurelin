# Aurelin Assist

Aurelin Assist gives coding agents local application context and debugging
evidence. It is a developer tool, not an AI feature served to application users.
The package provides a standalone `aurelin-assist` executable; generated apps
do not depend on it at runtime.

## Install and use

Until this package is published, install it from this repository:

```sh
cargo install --path crates/aurelin-assist
cd /path/to/your/application
aurelin-assist install --playwright
aurelin-assist doctor
```

Omit `--playwright` to install only Aurelin's read-only diagnostics. The installer
does not run npm, install browsers, launch debuggers, or change global settings.
An MCP client may launch the registered commands after discovering configuration;
Playwright's `npx` command can download its pinned package at that point.

Installation writes:

- a managed block in `AGENTS.md` and `CLAUDE.md`;
- `.agents/skills/aurelin-debugging/SKILL.md`, discovered by OpenCode and other
  clients supporting that skill location, plus the same skill under
  `.claude/skills/aurelin-debugging/SKILL.md` for Claude Code;
- `.mcp.json` for clients using `mcpServers`;
- `opencode.json`, using OpenCode V2's `mcp.servers` format.

`aurelin-assist update` refreshes managed blocks. Existing text outside those
blocks and existing MCP server entries are preserved, including customizations
to Assist or Playwright. Installation preflights all resources and replaces each
file atomically; it is not a multi-file transaction. An I/O failure can leave
some resources refreshed and others unchanged. Rerun after correcting the error.
Updates do not upgrade the executable: install the desired package version first.

Malformed JSON/markers and symlinked destinations are refused rather than
rewritten. JSONC and nested `.opencode` configuration are deliberately not
rewritten or shadowed. If these exist, register the server manually in that
configuration instead; managed resource installation currently requires a
project without those conflicting configuration files.

OpenCode reloads configuration automatically (`/reload` forces a reload).
Other clients may need their own reload/restart. Assist registrations contain
the selected application's absolute path. If you move the application, adjust
those entries manually; update preserves existing entries rather than guessing
whether they are user-owned. Configuration may also inherit other servers from
ancestor/global client configuration; Assist does not alter them.

## Manual MCP registration

OpenCode V2:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "aurelin-assist": {
        "type": "local",
        "command": ["aurelin-assist", "--project", "/absolute/path/to/app", "mcp"],
        "cwd": "/absolute/path/to/app"
      }
    }
  }
}
```

For other stdio MCP clients, use command `aurelin-assist` and arguments
`["--project", "/absolute/path/to/app", "mcp"]`. No HTTP endpoint is opened.
The same `--project` option works for install, update, and doctor. Shell
completions are available via `aurelin-assist completions bash` (or your shell).

### Tools

| Tool | Evidence |
| --- | --- |
| `application_info` | Package identity and versions in the application's root `Cargo.lock`, including transitive dependencies; no environment snapshot |
| `read_log_entries` | 1–100 complete JSON log entries, in chronological order |
| `last_error` | Latest ERROR within the bounded log window, or an empty array |
| `database_schema` | Tables/views and column metadata from an explicit project-relative SQLite file |

Run Cargo to generate a missing lockfile. Workspace applications whose lockfile
lives above the selected package must currently consult that lockfile separately;
Assist does not traverse outside its selected root. Schema inspection supports
SQLite only, not remote database connections. It never returns application rows,
runs arbitrary SQL, creates missing databases, or applies migrations. Results
are capped at 200 tables/views and 200 columns per table.

This is a focused Boost-inspired release, not full Laravel Boost parity. It
does not provide Tinker, arbitrary command execution, database queries, automatic
Axum route discovery, a hosted semantic documentation service, or a Telescope UI.
Guidelines direct agents to project docs and version-specific upstream docs.

## Backend logs

New backend, React, Vue, and Leptos starters call framework telemetry after
loading their environment. Existing applications replace their subscriber
initialization with:

```rust,ignore
let environment = aurelin_framework::configuration::environment(std::path::Path::new("."))?;
aurelin_framework::telemetry::init(std::path::Path::new("."), &environment)?;
```

Set `ASSIST_LOG=true` locally and restart the application. Terminal logs stay on
stderr. JSONL files rotate daily in `storage/logs/aurelin.jsonl.YYYY-MM-DD`,
retaining at most seven files when new files are created. Retention is file-count
based, not a disk quota: a busy day's log may still be large. `RUST_LOG` configures
both terminal and file filtering; malformed filters fail with an error.

The reader examines at most 1 MiB across the newest seven files, skips malformed
or incomplete records, and returns at most 100 entries. An error outside that
window cannot be reported by `last_error`. File capture is off by default and
rejected unless `APP_ENV` is `local` or `testing` (missing `APP_ENV` means local).
Assist itself also refuses other environments at startup, using Aurelin's normal
dotenv/external-variable precedence.

These are development guardrails, not an operating-system sandbox or a substitute
for database credentials and file permissions. Use only trusted local checkouts.

Logs are **not automatically redacted**. Existing log mail delivery can include
email bodies, including sensitive links. Do not enable file capture with real
credentials/customer data; avoid logging sensitive fields and redact before
sharing. The log directory is created privately on Unix; an existing directory's
permissions remain the operator's responsibility. Keep logs out of source control
(new starters ignore `/storage/logs/`).

## Optional debugging companions

### Browser reproduction: Playwright

`install --playwright` registers `@playwright/mcp@0.0.86` with an isolated,
headless browser profile. Node.js 20+ and a compatible Playwright browser are
external prerequisites. Follow [Playwright MCP's setup](https://github.com/microsoft/playwright-mcp)
for browser availability and connection troubleshooting. Browser tools inspect
console errors, request/response details, accessibility snapshots, and screenshots.
Use test accounts; browser actions can mutate your application and some tools
execute arbitrary code. They are not covered by Assist's read-only guarantee.

Microsoft also recommends considering [Playwright CLI + skills](https://github.com/microsoft/playwright-cli)
for coding agents to reduce context overhead. This is an alternative external
workflow, not a second browser engine implemented by Aurelin.

### Async stalls: Tokio Console

New starters expose a `tokio-console` Cargo feature forwarding to the optional
framework integration. Existing apps can enable `aurelin-framework/tokio-console`.
The integration composes a console layer with terminal/file logging rather than
installing another global subscriber.

```sh
# Install manually if wanted:
cargo install --locked tokio-console

# From a generated app (with no other app instance using port 6669):
RUSTFLAGS="--cfg tokio_unstable" TOKIO_CONSOLE=true cargo run --features tokio-console -- serve

# From another terminal:
tokio-console
```

Tokio's `tracing` feature is enabled by the framework feature. The unstable cfg
is still required; without either prerequisite the app reports an actionable
error instead of silently producing no task telemetry. Changing `RUSTFLAGS`
causes dependency rebuilds. The console binds to `127.0.0.1:6669` only; never
forward its unauthenticated diagnostic port publicly. It shows task polling,
scheduling, and resource behavior, not a substitute for native stack debugging.

### Breakpoints: RustRover debugger MCP

RustRover 2026.3 exposes debugger operations through its own MCP server. In the
IDE, enable **Settings → Tools → MCP Server**, then use its **Auto-Configure**
flow for your supported agent. Use the IDE-provided connection details rather
than guessing a port. See [JetBrains' agentic debugging guide](https://www.jetbrains.com/help/idea/agentic-debugging.html).

This integration is setup guidance and an installed debugging skill, not an
Aurelin implementation of a debugger. RustRover remains separately installed
and configured. `rust-gdb`, `rust-lldb`, and CodeLLDB are alternatives. Debugger
expressions and resumed application code can cause side effects.

`doctor` reports project context, requested logging/console settings, and these
setup instructions. It does not claim external tools are installed or connected.

## Ownership and references

- `laravel/boost` owns AI guidelines, installer/updater, and MCP diagnostics;
  reference revision `c1c27ed55bf32e0c82f71663516f89e23eefde3e` (2026-11-25),
  particularly `src/Mcp/Boost.php`, `src/Mcp/Tools/LastError.php`, and its tests.
- `aurelin-assist` owns the corresponding local developer tooling. Its public
  library is shared by the executable and integration tests. `rmcp` owns MCP
  transport and protocol handling; Aurelin does not reimplement JSON-RPC.
- Laravel Framework logging maps to `aurelin-framework::telemetry`, composed
  from tracing-subscriber and tracing-appender. Tokio Console is optional.
- Starters contain only application configuration, a thin telemetry call, and
  Cargo feature selection, copied by the existing scaffolder. No Assist runtime
  logic is copied into applications.
- Verified dependencies: `rmcp` 3.11.0 (Apache-2.0, Rust 1.88),
  `console-subscriber` 0.5.0 (MIT, Rust 1.74), `tracing-appender` 0.2.5
  (MIT, Rust 1.63), and Playwright MCP 0.0.86 (Apache-2.0, Node.js 20+).
- [Laravel Boost](https://laravel.com/framework/docs/boost),
  [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk),
  [Tokio Console](https://github.com/tokio-rs/console),
  [RustRover debugging, November 2026](https://blog.jetbrains.com/rust/2026/11/10/debugging-rust/),
  [OpenCode MCP configuration](https://opencode.ai/docs/mcp-servers).
