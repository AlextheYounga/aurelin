mod scaffold;

use std::{io, path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

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
        /// Destination directory; must not already exist
        path: PathBuf,
        /// Local aurelin-framework crate (required until Aurelin is published)
        #[arg(long)]
        framework_path: PathBuf,
        /// Cargo package name; defaults to the destination directory name
        #[arg(long)]
        name: Option<String>,
        /// Install a React or Vue Inertia authentication starter
        #[arg(long, value_enum)]
        frontend: Option<scaffold::Frontend>,
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
        Commands::New { path, framework_path, name, frontend } => {
            scaffold::create(&path, &framework_path, name.as_deref(), frontend)?;
            if frontend.is_some() {
                eprintln!("Created {}. Run `npm install`, then `cargo run` inside it.", path.display());
            } else {
                eprintln!("Created {}. Run `cargo run` inside it to start the server.", path.display());
            }
        }
        Commands::Completions { shell } => {
            generate(shell, &mut Cli::command(), "aurelin", &mut io::stdout());
        }
    }
    Ok(())
}
