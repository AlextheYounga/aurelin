mod scaffold;

use std::{io, io::IsTerminal, path::PathBuf, process::ExitCode};

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use dialoguer::{Input, Select};

#[derive(Parser)]
#[command(name = "aurelin", version, about = "Create Laravel-inspired Rust applications")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create an HTTP application, optionally with the React or Vue authentication starter
    New {
        /// Destination directory; prompted for when omitted in an interactive terminal
        path: Option<PathBuf>,
        /// Use a local aurelin-framework crate instead of the published Aurelin crates
        #[arg(long)]
        framework_path: Option<PathBuf>,
        /// Cargo package name; defaults to the destination directory name
        #[arg(long)]
        name: Option<String>,
        /// Install a React or Vue Inertia authentication starter
        #[arg(long, value_enum)]
        frontend: Option<scaffold::Frontend>,
        /// Never prompt; omitted options use their non-interactive defaults
        #[arg(long)]
        no_interaction: bool,
    },
    /// Generate a shell completion script on stdout
    Completions { shell: Shell },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::New { path, framework_path, name, frontend, no_interaction } => {
            let interactive = !no_interaction && io::stdin().is_terminal() && io::stderr().is_terminal();
            let path = destination(path, interactive)?;
            let frontend = select_frontend(frontend, interactive)?;
            let framework = scaffold::FrameworkSource::resolve(framework_path)?;
            scaffold::create(&path, &framework, name.as_deref(), frontend)?;
            if frontend.is_some() {
                eprintln!(
                    "Created {}. Run `npm install`, `cargo aurelin migrate`, then `cargo aurelin dev`.",
                    path.display()
                );
            } else {
                eprintln!("Created {}. Run `cargo aurelin serve` to start the server.", path.display());
            }
        }
        Commands::Completions { shell } => {
            generate(shell, &mut Cli::command(), "aurelin", &mut io::stdout());
        }
    }
    Ok(())
}

fn destination(path: Option<PathBuf>, interactive: bool) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(path);
    }
    if !interactive {
        bail!("destination is required when not running interactively; use `aurelin new <path>`");
    }
    let path = Input::<String>::new().with_prompt("Application path").interact_text()?;
    Ok(PathBuf::from(path))
}

fn select_frontend(frontend: Option<scaffold::Frontend>, interactive: bool) -> Result<Option<scaffold::Frontend>> {
    if frontend.is_some() || !interactive {
        return Ok(frontend);
    }
    let choices = ["React with authentication", "Vue with authentication", "Backend only"];
    Ok(match Select::new().with_prompt("Application starter").items(&choices).default(0).interact()? {
        0 => Some(scaffold::Frontend::React),
        1 => Some(scaffold::Frontend::Vue),
        _ => None,
    })
}
