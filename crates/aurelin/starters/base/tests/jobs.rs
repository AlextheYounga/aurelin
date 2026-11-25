use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use aurelin_framework::queue::{self, BoxDynError, WorkerBuilder, WorkerContext};
use aurelin_framework::{
    events::EventDispatcher,
    mail::{MailMessage, Mailer},
    queue::TaskSink,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[tokio::test]
async fn application_job_is_dispatched_and_processed_by_a_worker() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let database_url = format!(
        "sqlite://{}/queue.sqlite?mode=rwc",
        directory.path().display()
    );
    __AURELIN_CRATE__::database::migrate(&database_url).await?;
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

#[tokio::test]
async fn registered_workers_process_all_job_types_and_stop_on_signal() -> Result<(), Box<dyn Error>>
{
    use __AURELIN_CRATE__::jobs::{ExampleJob, ExampleListenerJob};
    let directory = tempfile::tempdir()?;
    let url = format!(
        "sqlite://{}/queue.sqlite?mode=rwc",
        directory.path().display()
    );
    __AURELIN_CRATE__::database::migrate(&url).await?;
    let mut examples = queue::sqlite::<ExampleJob>(&url).await?;
    let listeners = queue::sqlite::<ExampleListenerJob>(&url).await?;
    let mut mail = queue::sqlite::<MailMessage>(&url).await?;
    examples
        .push(ExampleJob {
            message: "example".into(),
        })
        .await?;
    mail.push(MailMessage::new("subject", "body").to("ada@example.com")?)
        .await?;
    let mut events = EventDispatcher::builder();
    events.listen_queued(listeners, |message: &String| ExampleListenerJob {
        message: message.clone(),
    });
    events.build().dispatch("listener".to_owned()).await?;
    let database = aurelin_framework::database::connect(&url).await?;
    let shutdown = async move {
        loop {
            let query = Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM Jobs WHERE status = 'Done'",
            );
            if let Ok(Some(row)) = database.query_one(query).await
                && row.try_get::<i64>("", "count").unwrap_or_default() == 3
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        __AURELIN_CRATE__::jobs::work_until(&url, Mailer::log(), shutdown),
    )
    .await??;
    Ok(())
}
