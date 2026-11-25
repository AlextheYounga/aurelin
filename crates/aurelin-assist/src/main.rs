use std::{io, path::PathBuf, process::ExitCode, time::Duration};

use anyhow::Result;
use aurelin_assist::{install, project::Project, server::Assist};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use rmcp::{ServiceExt, transport::stdio};
use tokio::{runtime::Builder, signal::ctrl_c};

#[derive(Parser)]
#[command(version, about = "AI context and local debugging for Aurelin applications")]
struct Cli {
    /// Application directory (never a machine-wide configuration directory)
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install managed guidelines, skills, and project MCP configuration
    Install {
        /// Also register the optional Playwright browser MCP companion
        #[arg(long)]
        playwright: bool,
    },
    /// Refresh installed resources while preserving user-owned content
    Update,
    /// Serve local read-only tools over MCP stdio (stdout is protocol-only)
    Mcp,
    /// Report project context and optional debugger setup instructions
    Doctor,
    /// Generate shell completions
    Completions { shell: Shell },
}

fn main() -> ExitCode {
    match run_command(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_command(cli: Cli) -> Result<()> {
    let runtime = Builder::new_multi_thread().enable_all().build()?;
    let result = runtime.block_on(run(cli));
    // Tokio stdin uses an uncancellable blocking read. Bound runtime shutdown
    // so Ctrl+C can exit even while an MCP client keeps its stdin pipe open.
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}

async fn run(cli: Cli) -> Result<()> {
    if let Command::Completions { shell } = cli.command {
        generate(shell, &mut Cli::command(), "aurelin-assist", &mut io::stdout());
        return Ok(());
    }
    let project = Project::open(&cli.project)?;
    match cli.command {
        Command::Install { playwright } => {
            install::install(&project, playwright)?;
            eprintln!(
                "Installed Aurelin Assist. OpenCode reloads configuration automatically; other clients may need a reload."
            );
        }
        Command::Update => {
            install::update(&project)?;
            eprintln!("Updated Aurelin Assist; user-owned instructions and MCP entries were preserved.");
        }
        Command::Mcp => {
            tokio::select! {
                result = async {
                    let service = Assist::new(project).serve(stdio()).await?;
                    service.waiting().await?;
                    Ok::<_, anyhow::Error>(())
                } => { result?; }
                result = ctrl_c() => { result?; }
            }
        }
        Command::Doctor => {
            println!("{}", serde_json::to_string_pretty(&project.info()?)?);
            println!("\n{}", include_str!("../resources/debugging.md"));
        }
        Command::Completions { .. } => {}
    }
    Ok(())
}
