use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use aurelin_framework::queue::{self, BoxDynError, TaskSink, WorkerBuilder, WorkerContext};

#[tokio::test]
async fn sqlite_queue_dispatches_a_job_to_a_worker() -> Result<(), Box<dyn Error>> {
    let database = tempfile::NamedTempFile::new()?;
    let database_url = format!("sqlite://{}?mode=rwc", database.path().display());
    let mut queue = queue::sqlite::<usize>(&database_url).await?;
    queue.push(42).await?;

    let executions = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&executions);
    let worker = WorkerBuilder::new("queue-test").backend(queue).build(move |job: usize, worker: WorkerContext| {
        let observed = Arc::clone(&observed);
        async move {
            observed.store(job, Ordering::SeqCst);
            worker.stop()?;
            Ok::<(), BoxDynError>(())
        }
    });

    worker.run().await?;
    assert_eq!(executions.load(Ordering::SeqCst), 42);
    Ok(())
}
