Optional debugging companions

- Backend logs: set ASSIST_LOG=true in your local .env and restart the app.
  New starters use framework telemetry. Logs rotate daily, retaining seven files
  under storage/logs. File capture is disabled by default and outside local/testing.
- Browser: `aurelin-assist install --playwright` registers a pinned Playwright MCP
  command with an isolated headless profile. Starting that companion may download
  its npm package; Node.js 20+ and a compatible browser are external prerequisites.
  Alternatively use Microsoft's Playwright CLI + skills:
  https://github.com/microsoft/playwright-cli
- Async tasks: install `cargo install --locked tokio-console` yourself, enable
  your app's tokio-console feature, and run:
  RUSTFLAGS="--cfg tokio_unstable" TOKIO_CONSOLE=true cargo run --features tokio-console -- serve
  Then run `tokio-console`. The diagnostic endpoint binds only to 127.0.0.1:6669.
- Native debugger: RustRover 2026.3 can expose breakpoints, stepping, and variables
  through its own MCP server. Enable Settings → Tools → MCP Server and use the
  IDE's Auto-Configure flow. Assist does not install RustRover or guess its port.
  See https://www.jetbrains.com/help/idea/agentic-debugging.html.
  rust-gdb, rust-lldb, and CodeLLDB remain alternatives for debug builds.

Doctor reports configuration intent, not whether external tools are installed,
connected, or healthy. Assist never installs these companions automatically.
Logs, browser sessions, and debugger expressions may expose sensitive data;
use test accounts and keep diagnostic ports private.
