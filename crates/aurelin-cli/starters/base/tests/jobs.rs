use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use aurelin_framework::queue::{self, BoxDynError, WorkerBuilder, WorkerContext};

#[tokio::test]
async fn application_job_is_dispatched_and_processed_by_a_worker() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}/queue.sqlite?mode=rwc",
        directory.path().display()
    );
    __AURELIN_CRATE__::jobs::dispatch(
        &database_url,
        __AURELIN_CRATE__::jobs::ExampleJob {
            message: "hello from the queue".to_owned(),
        },
    )
    .await?;

    let queue = queue::sqlite::<__AURELIN_CRATE__::jobs::ExampleJob>(&database_url).await?;
    let processed = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&processed);
    WorkerBuilder::new("application-job-test")
        .backend(queue)
        .build(
            move |job: __AURELIN_CRATE__::jobs::ExampleJob, worker: WorkerContext| {
                let observed = Arc::clone(&observed);
                async move {
                    assert_eq!(job.message, "hello from the queue");
                    observed.store(true, Ordering::SeqCst);
                    worker.stop()?;
                    Ok::<(), BoxDynError>(())
                }
            },
        )
        .run()
        .await?;

    assert!(processed.load(Ordering::SeqCst));
    Ok(())
}
