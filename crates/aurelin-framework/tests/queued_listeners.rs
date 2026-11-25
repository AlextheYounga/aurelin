use std::{
    convert::Infallible,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use aurelin_framework::{
    events::EventDispatcher,
    mail::{MailMessage, Mailer},
    queue::{self, BoxDynError, TaskSink, WorkerBuilder, WorkerContext},
};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

struct OrderPlaced {
    number: u32,
}

#[derive(Serialize, Deserialize)]
struct ListenerJob {
    number: u32,
}

#[derive(Serialize, Deserialize)]
struct ExampleJob {
    number: u32,
}

#[tokio::test]
async fn independent_sqlite_workers_execute_example_mail_and_queued_listener_jobs() -> Result<(), Box<dyn Error>> {
    let database = tempfile::NamedTempFile::new()?;
    let url = format!("sqlite://{}?mode=rwc", database.path().display());
    let pool = apalis_sqlite::SqlitePool::connect(&url).await?;
    apalis_sqlite::SqliteStorage::setup(&pool).await?;
    pool.close().await;

    let listener_queue = queue::sqlite::<ListenerJob>(&url).await?;
    let mut example_queue = queue::sqlite::<ExampleJob>(&url).await?;
    let mut mail_queue = queue::sqlite::<MailMessage>(&url).await?;
    let mut events = EventDispatcher::builder();
    events.listen_queued(listener_queue.clone(), |event: &OrderPlaced| ListenerJob { number: event.number });
    events.build().dispatch(OrderPlaced { number: 42 }).await?;
    example_queue.push(ExampleJob { number: 7 }).await?;
    mail_queue.push(MailMessage::new("Order", "Queued").to("ada@example.com")?).await?;

    let observed = Arc::new(AtomicUsize::new(0));
    let listener_observed = Arc::clone(&observed);
    let listener =
        WorkerBuilder::new("listener").backend(listener_queue).build(move |job: ListenerJob, worker: WorkerContext| {
            let observed = Arc::clone(&listener_observed);
            async move {
                observed.fetch_add(job.number as usize, Ordering::SeqCst);
                worker.stop()?;
                Ok::<(), BoxDynError>(())
            }
        });
    let example_observed = Arc::clone(&observed);
    let example =
        WorkerBuilder::new("example").backend(example_queue).build(move |job: ExampleJob, worker: WorkerContext| {
            let observed = Arc::clone(&example_observed);
            async move {
                observed.fetch_add(job.number as usize, Ordering::SeqCst);
                worker.stop()?;
                Ok::<(), BoxDynError>(())
            }
        });
    let mail_observed = Arc::clone(&observed);
    let mail =
        WorkerBuilder::new("mail").backend(mail_queue).build(move |message: MailMessage, worker: WorkerContext| {
            let observed = Arc::clone(&mail_observed);
            async move {
                Mailer::log().send(message).await?;
                observed.fetch_add(1, Ordering::SeqCst);
                worker.stop()?;
                Ok::<(), BoxDynError>(())
            }
        });
    timeout(Duration::from_secs(15), async { tokio::try_join!(listener.run(), example.run(), mail.run()) }).await??;
    assert_eq!(observed.load(Ordering::SeqCst), 50);
    Ok(())
}

#[tokio::test]
async fn queued_listener_enqueue_failure_stops_later_synchronous_listeners() -> Result<(), Box<dyn Error>> {
    let database = tempfile::NamedTempFile::new()?;
    let url = format!("sqlite://{}?mode=rwc", database.path().display());
    let pool = apalis_sqlite::SqlitePool::connect(&url).await?;
    apalis_sqlite::SqliteStorage::setup(&pool).await?;
    pool.close().await;
    let queue = queue::sqlite::<ListenerJob>(&url).await?;
    queue.pool().close().await;
    let called = Arc::new(AtomicUsize::new(0));
    let mut events = EventDispatcher::builder();
    events.listen_queued(queue, |event: &OrderPlaced| ListenerJob { number: event.number });
    let captured = Arc::clone(&called);
    events.listen(move |_: Arc<OrderPlaced>| {
        let captured = Arc::clone(&captured);
        async move {
            captured.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>(())
        }
    });
    assert!(events.build().dispatch(OrderPlaced { number: 1 }).await.is_err());
    assert_eq!(called.load(Ordering::SeqCst), 0);
    Ok(())
}
