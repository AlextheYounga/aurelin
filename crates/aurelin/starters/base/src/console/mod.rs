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
    /// Run scheduled tasks that are due now
    #[command(name = "schedule:run")]
    ScheduleRun,
    /// List failed queue jobs
    #[command(name = "queue:failed")]
    QueueFailed {
        /// Only list failures from this queue
        #[arg(long)]
        queue: Option<String>,
    },
    /// Retry failed queue jobs
    #[command(name = "queue:retry")]
    QueueRetry {
        #[arg(num_args = 1..)]
        ids: Vec<String>,
    },
    /// Delete failed queue jobs
    #[command(name = "queue:forget")]
    QueueForget {
        #[arg(num_args = 1..)]
        ids: Vec<String>,
    },
    /// Delete all matching failed queue jobs
    #[command(name = "queue:flush")]
    QueueFlush {
        /// Retain jobs that failed within this many hours
        #[arg(long)]
        hours: Option<u64>,
        /// Only delete failures from this queue
        #[arg(long)]
        queue: Option<String>,
        /// Run the command in production
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(environment: HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        None | Some(Command::List) => {
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
        Some(Command::Serve) => crate::serve(environment).await,
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
        Some(Command::QueueWork) => {
            let mailer = aurelin_framework::mail::Mailer::from_environment(&environment)?;
            crate::jobs::work(database_url(&environment), mailer).await
        }
        Some(Command::ScheduleRun) => {
            aurelin_framework::scheduling::commands::run(&crate::schedule::schedule()?).await?;
            Ok(())
        }
        Some(Command::QueueFailed { queue }) => {
            aurelin_framework::queue::commands::failed(database_url(&environment), queue.as_deref())
                .await
                .map_err(Into::into)
        }
        Some(Command::QueueRetry { ids }) => {
            aurelin_framework::queue::commands::retry(database_url(&environment), &ids)
                .await
                .map_err(Into::into)
        }
        Some(Command::QueueForget { ids }) => {
            aurelin_framework::queue::commands::forget(database_url(&environment), &ids)
                .await
                .map_err(Into::into)
        }
        Some(Command::QueueFlush {
            hours,
            queue,
            force,
        }) => {
            ensure_safe_queue_flush(&environment, force)?;
            aurelin_framework::queue::commands::flush(
                database_url(&environment),
                hours,
                queue.as_deref(),
            )
            .await
            .map_err(Into::into)
        }
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

fn ensure_safe_queue_flush(
    environment: &HashMap<String, String>,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    if environment.get("APP_ENV").map(String::as_str) == Some("production") && !force {
        return Err("refusing to flush failed jobs in production without --force".into());
    }
    Ok(())
}

fn database_url(environment: &HashMap<String, String>) -> &str {
    environment
        .get("DATABASE_URL")
        .map(String::as_str)
        .unwrap_or("sqlite://database/database.sqlite?mode=rwc")
}
