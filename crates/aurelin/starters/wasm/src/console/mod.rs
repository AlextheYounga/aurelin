use std::{collections::HashMap, error::Error};

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"), version, about = "Aurelin application commands")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List available application commands
    List,
    /// Start the HTTP server
    Serve,
    /// Apply pending database migrations
    Migrate {
        /// Run the command in production
        #[arg(long)]
        force: bool,
    },
    /// Show applied and pending database migrations
    #[command(name = "migrate:status")]
    MigrateStatus,
    /// Roll back the most recently applied migration
    #[command(name = "migrate:rollback")]
    MigrateRollback {
        /// Run the command in production
        #[arg(long)]
        force: bool,
    },
    /// Drop all tables and apply every migration
    #[command(name = "migrate:fresh")]
    MigrateFresh {
        /// Run the command in production
        #[arg(long)]
        force: bool,
    },
    /// Process queued jobs
    #[command(name = "queue:work")]
    QueueWork,
}

pub async fn run(environment: HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        None | Some(Command::Serve) => crate::serve(environment).await,
        Some(Command::List) => {
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
        Some(Command::Migrate { force }) => {
            ensure_safe_environment(&environment, force)?;
            crate::database::migrate(database_url(&environment)).await
        }
        Some(Command::MigrateStatus) => {
            crate::database::migration_status(database_url(&environment)).await
        }
        Some(Command::MigrateRollback { force }) => {
            ensure_safe_environment(&environment, force)?;
            crate::database::rollback(database_url(&environment)).await
        }
        Some(Command::MigrateFresh { force }) => {
            ensure_safe_environment(&environment, force)?;
            crate::database::fresh(database_url(&environment)).await
        }
        Some(Command::QueueWork) => crate::jobs::work(database_url(&environment)).await,
    }
}

fn ensure_safe_environment(
    environment: &HashMap<String, String>,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    if environment.get("APP_ENV").map(String::as_str) == Some("production") && !force {
        return Err("refusing to run migrations in production without --force".into());
    }
    Ok(())
}

fn database_url(environment: &HashMap<String, String>) -> &str {
    environment
        .get("DATABASE_URL")
        .map(String::as_str)
        .unwrap_or("sqlite://database/database.sqlite?mode=rwc")
}
