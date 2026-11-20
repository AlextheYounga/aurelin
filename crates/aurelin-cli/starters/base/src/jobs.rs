use std::error::Error;

use aurelin_framework::queue::{self, BoxDynError, TaskSink, WorkerBuilder, WorkerContext};
use serde::{Deserialize, Serialize};

/// Replace this example with an application job of your own.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExampleJob {
    pub message: String,
}

pub async fn dispatch(database_url: &str, job: ExampleJob) -> Result<(), Box<dyn Error>> {
    let mut queue = queue::sqlite(database_url).await?;
    queue.push(job).await?;
    Ok(())
}

pub async fn work(database_url: &str) -> Result<(), Box<dyn Error>> {
    let queue = queue::sqlite::<ExampleJob>(database_url).await?;
    WorkerBuilder::new("default")
        .backend(queue)
        .build(handle)
        .run()
        .await?;
    Ok(())
}

async fn handle(job: ExampleJob, _worker: WorkerContext) -> Result<(), BoxDynError> {
    tracing::info!(message = job.message, "processed example job");
    Ok(())
}
