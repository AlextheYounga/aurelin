use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use aurelin_framework::{
    events::EventDispatcher,
    mail::{MailMessage, Mailer, MessageFailed},
    notifications::{AnonymousRecipient, Channel, Notification, NotificationDispatcher, NotificationError},
    queue::{self, BoxDynError, WorkerBuilder, WorkerContext},
};

struct Notice;

impl Notification<AnonymousRecipient> for Notice {
    fn via(&self, _: &AnonymousRecipient) -> Result<Vec<Channel>, NotificationError> {
        Ok(vec![Channel::Mail])
    }
    fn to_mail(&self, _: &AnonymousRecipient) -> Result<MailMessage, NotificationError> {
        Ok(MailMessage::new("Notice", "Content"))
    }
}

#[tokio::test]
async fn queued_notification_transport_failure_is_reported_by_real_worker() -> Result<(), Box<dyn Error>> {
    let db = tempfile::NamedTempFile::new()?;
    let url = format!("sqlite://{}?mode=rwc", db.path().display());
    let pool = apalis_sqlite::SqlitePool::connect(&url).await?;
    apalis_sqlite::SqliteStorage::setup(&pool).await?;
    pool.close().await;
    let mut queue = queue::sqlite::<MailMessage>(&url).await?;
    NotificationDispatcher::new(Mailer::log())
        .queue(&AnonymousRecipient::mail("ada@example.com"), &Notice, &mut queue)
        .await?;

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let environment = HashMap::from([
        ("MAIL_MAILER".into(), "smtp".into()),
        ("MAIL_HOST".into(), "127.0.0.1".into()),
        ("MAIL_PORT".into(), port.to_string()),
        ("MAIL_ENCRYPTION".into(), "none".into()),
        ("MAIL_FROM_ADDRESS".into(), "default@example.com".into()),
    ]);
    let failures = Arc::new(AtomicUsize::new(0));
    let captured = Arc::clone(&failures);
    let mut events = EventDispatcher::builder();
    events.listen(move |_: Arc<MessageFailed>| {
        let captured = Arc::clone(&captured);
        async move {
            captured.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>(())
        }
    });
    let mailer = Mailer::from_environment(&environment)?.with_events(events.build());
    WorkerBuilder::new("notification-failure")
        .backend(queue)
        .build(move |message: MailMessage, worker: WorkerContext| {
            let mailer = mailer.clone();
            async move {
                worker.stop()?;
                mailer.send(message).await?;
                Ok::<(), BoxDynError>(())
            }
        })
        .run()
        .await?;
    assert_eq!(failures.load(Ordering::SeqCst), 1);
    Ok(())
}
