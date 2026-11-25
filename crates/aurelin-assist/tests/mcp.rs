mod common;

use std::{fs, path::Path, process::Stdio, time::Duration};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdout, Command},
    time::timeout,
};

#[tokio::test]
async fn stdio_client_can_discover_tools_and_diagnose_a_backend_error_without_stdout_noise() -> Result<()> {
    let directory = common::application()?;
    let root = directory.path();
    assert!(common::cli(root, &["--help"])?.status.success());
    assert!(common::cli(root, &["--version"])?.status.success());
    fs::create_dir_all(root.join("storage/logs"))?;
    fs::write(
        root.join("storage/logs/aurelin.jsonl.2026-11-25"),
        "{\"level\":\"ERROR\",\"fields\":{\"message\":\"connection refused\",\"request_id\":\"req-42\"}}\n",
    )?;
    let mut child = start_server(root)?;
    let mut input = child.stdin.take().context("child stdin")?;
    let mut output = BufReader::new(child.stdout.take().context("child stdout")?);
    input
        .write_all(
            format!(
                "{}\n",
                json!({
                    "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                        "protocolVersion": "2025-11-25", "capabilities": {},
                        "clientInfo": {"name": "integration-test", "version": "1"}
                    }
                })
            )
            .as_bytes(),
        )
        .await?;
    let initialized = receive(&mut output).await?;
    assert_eq!(initialized["result"]["serverInfo"]["name"], "aurelin-assist");
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await?;
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n").await?;
    let tools = receive(&mut output).await?;
    let tools = tools["result"]["tools"].as_array().context("tool catalog")?;
    for name in ["application_info", "last_error", "read_log_entries", "database_schema"] {
        let tool = tools.iter().find(|tool| tool["name"] == name).context("missing diagnostic tool")?;
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
    for (id, name, arguments) in [
        (3, "application_info", json!({})),
        (4, "last_error", json!({})),
        (5, "read_log_entries", json!({"count": 0})),
        (6, "database_schema", json!({"database": "missing.sqlite"})),
    ] {
        input
            .write_all(
                format!(
                    "{}\n",
                    json!({
                        "jsonrpc": "2.0", "id": id, "method": "tools/call",
                        "params": {"name": name, "arguments": arguments}
                    })
                )
                .as_bytes(),
            )
            .await?;
        let response = receive(&mut output).await?;
        assert_eq!(response["id"], id);
        assert!(!response.to_string().contains("must-not-leak"));
        match name {
            "application_info" => assert!(
                response["result"]["content"][0]["text"].as_str().context("info content")?.contains("example-app")
            ),
            "last_error" => {
                assert!(response["result"]["content"][0]["text"].as_str().context("error content")?.contains("req-42"));
            }
            _ => assert_eq!(response["result"]["isError"], true),
        }
    }
    assert!(!root.join("missing.sqlite").exists());
    drop(input);
    assert!(timeout(Duration::from_secs(10), child.wait()).await??.success());
    Ok(())
}

async fn receive(output: &mut BufReader<ChildStdout>) -> Result<Value> {
    let mut line = String::new();
    let bytes = timeout(Duration::from_secs(10), output.read_line(&mut line)).await??;
    anyhow::ensure!(bytes != 0, "server exited without a response");
    Ok(serde_json::from_str(&line)?)
}

fn start_server(root: &Path) -> Result<Child> {
    Ok(Command::new(env!("CARGO_BIN_EXE_aurelin-assist"))
        .arg("mcp")
        .current_dir(root)
        .env_clear()
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?)
}

#[cfg(unix)]
#[tokio::test]
async fn ctrl_c_exits_successfully_even_when_the_client_keeps_stdin_open() -> Result<()> {
    let directory = common::application()?;
    let mut child = start_server(directory.path())?;
    let mut input = child.stdin.take().context("child stdin")?;
    let mut output = BufReader::new(child.stdout.take().context("child stdout")?);
    input
        .write_all(
            format!(
                "{}\n",
                json!({
                    "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                        "protocolVersion": "2025-11-25", "capabilities": {},
                        "clientInfo": {"name": "signal-test", "version": "1"}
                    }
                })
            )
            .as_bytes(),
        )
        .await?;
    assert_eq!(receive(&mut output).await?["result"]["serverInfo"]["name"], "aurelin-assist");
    let pid = child.id().context("child pid")?;
    assert!(Command::new("kill").args(["-INT", &pid.to_string()]).status().await?.success());
    assert!(timeout(Duration::from_secs(5), child.wait()).await??.success());
    // Intentionally retain the input pipe until after the child has exited.
    drop(input);
    Ok(())
}
