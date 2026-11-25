use std::{error::Error, future::Future, io};

use aurelin_framework::{
    mail::{MailMessage, Mailer},
    queue::{self, BoxDynError, Monitor, TaskBuilder, TaskSink, WorkerBuilder, WorkerContext},
};
use serde::{Deserialize, Serialize};

/// Replace this example with an application job of your own.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExampleJob {
    pub message: String,
}

pub async fn dispatch(database_url: &str, job: ExampleJob) -> Result<(), Box<dyn Error>> {
    let mut queue = queue::sqlite(database_url).await?;
    queue
        .push_task(TaskBuilder::new(job).max_attempts(1).build())
        .await?;
    Ok(())
}

/// An application-owned queued event listener payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExampleListenerJob {
    pub message: String,
}

pub async fn work(database_url: &str, mailer: Mailer) -> Result<(), Box<dyn Error>> {
    work_until(database_url, mailer, tokio::signal::ctrl_c()).await
}

/// Run the application's registered workers until the supplied shutdown signal.
pub async fn work_until(
    database_url: &str,
    mailer: Mailer,
    signal: impl Future<Output = io::Result<()>> + Send,
) -> Result<(), Box<dyn Error>> {
    let examples = queue::sqlite::<ExampleJob>(database_url).await?;
    let listeners = queue::sqlite::<ExampleListenerJob>(database_url).await?;
    let mail = queue::sqlite::<MailMessage>(database_url).await?;
    Monitor::new()
        .register(move |_| {
            WorkerBuilder::new("examples")
                .backend(examples.clone())
                .build(handle)
        })
        .register(move |_| {
            WorkerBuilder::new("listeners")
                .backend(listeners.clone())
                .build(handle_listener)
        })
        .register(move |_| {
            let mailer = mailer.clone();
            WorkerBuilder::new("mail")
                .backend(mail.clone())
                .build(move |message: MailMessage| {
                    let mailer = mailer.clone();
                    async move { mailer.send(message).await.map_err(BoxDynError::from) }
                })
        })
        .run_with_signal(signal)
        .await?;
    Ok(())
}

async fn handle_listener(
    job: ExampleListenerJob,
    _worker: WorkerContext,
) -> Result<(), BoxDynError> {
    tracing::info!(message = job.message, "processed example listener");
    Ok(())
}

async fn handle(job: ExampleJob, _worker: WorkerContext) -> Result<(), BoxDynError> {
    tracing::info!(message = job.message, "processed example job");
    Ok(())
}
