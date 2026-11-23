mod scaffold;

use std::{
    io,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use console::style;
use dialoguer::{Input, Select, theme::ColorfulTheme};

#[derive(Parser)]
#[command(name = "aurelin", version, about = "Create Laravel-inspired Rust applications")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create an HTTP application, optionally with a browser frontend
    New {
        /// Destination directory; prompted for when omitted in an interactive terminal
        path: Option<PathBuf>,
        /// Use a local aurelin-framework crate instead of the published Aurelin crates
        #[arg(long)]
        framework_path: Option<PathBuf>,
        /// Cargo package name; defaults to the destination directory name
        #[arg(long)]
        name: Option<String>,
        /// Install a React, Vue, or full-stack Rust/WASM frontend
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
            eprintln!("{} {error:#}", style("error:").for_stderr().red().bold());
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
            print_next_steps(&path, frontend);
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
    let theme = ColorfulTheme::default();
    let path = Input::<String>::with_theme(&theme).with_prompt("Application path").interact_text()?;
    Ok(PathBuf::from(path))
}

fn select_frontend(frontend: Option<scaffold::Frontend>, interactive: bool) -> Result<Option<scaffold::Frontend>> {
    if frontend.is_some() || !interactive {
        return Ok(frontend);
    }
    let theme = ColorfulTheme::default();
    let choices = ["React with authentication", "Vue with authentication", "Rust/WASM with Leptos", "Backend only"];
    Ok(match Select::with_theme(&theme).with_prompt("Application starter").items(&choices).default(0).interact()? {
        0 => Some(scaffold::Frontend::React),
        1 => Some(scaffold::Frontend::Vue),
        2 => Some(scaffold::Frontend::Wasm),
        _ => None,
    })
}

fn print_next_steps(path: &Path, frontend: Option<scaffold::Frontend>) {
    eprintln!("{} Created {}", style("✔").for_stderr().green(), style(path.display()).for_stderr().bold());
    eprintln!();
    eprintln!("{}", style("Next steps").for_stderr().cyan().bold());
    eprintln!("  {} {}", style("cd").for_stderr().cyan(), path.display());

    if frontend == Some(scaffold::Frontend::Wasm) {
        for command in ["cargo aurelin migrate", "cargo leptos watch"] {
            eprintln!("  {}", style(command).for_stderr().cyan());
        }
    } else if frontend.is_some() {
        for command in ["npm install", "cargo aurelin migrate", "cargo aurelin dev"] {
            eprintln!("  {}", style(command).for_stderr().cyan());
        }
    } else {
        eprintln!("  {}", style("cargo aurelin serve").for_stderr().cyan());
    }
}
