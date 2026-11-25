use aurelin_framework::{
    events::{EventDispatcher, ListenerControl},
    mail::{MailMessage, Mailer, MessageSending},
    notifications::{
        AnonymousRecipient, Channel, DispatchError, Notification, NotificationDispatcher, NotificationError,
        NotificationFailed, NotificationSending, NotificationSent,
    },
};
use std::{
    convert::Infallible,
    error::Error,
    sync::{Arc, Mutex},
};

struct Notice;
impl Notification<AnonymousRecipient> for Notice {
    fn via(&self, _: &AnonymousRecipient) -> Result<Vec<Channel>, NotificationError> {
        Ok(vec![Channel::Mail])
    }
    fn to_mail(&self, _: &AnonymousRecipient) -> Result<MailMessage, NotificationError> {
        Ok(MailMessage::new("subject", "sensitive content"))
    }
}

#[tokio::test]
async fn failed_synchronous_transport_emits_notification_failure() -> Result<(), Box<dyn Error>> {
    use std::{collections::HashMap, net::TcpListener};
    let socket = TcpListener::bind("127.0.0.1:0")?;
    let port = socket.local_addr()?.port();
    drop(socket);
    let config = HashMap::from([
        ("MAIL_MAILER".into(), "smtp".into()),
        ("MAIL_HOST".into(), "127.0.0.1".into()),
        ("MAIL_PORT".into(), port.to_string()),
        ("MAIL_ENCRYPTION".into(), "none".into()),
        ("MAIL_FROM_ADDRESS".into(), "default@example.com".into()),
    ]);
    let failed = Arc::new(Mutex::new(false));
    let observed = Arc::clone(&failed);
    let mut events = EventDispatcher::builder();
    events.listen(move |event: Arc<NotificationFailed>| {
        let observed = Arc::clone(&observed);
        async move {
            *observed.lock().unwrap() = event.0.recipient == "ada@example.com";
            Ok::<_, Infallible>(())
        }
    });
    let result = NotificationDispatcher::new(Mailer::from_environment(&config)?)
        .with_events(events.build())
        .send(&AnonymousRecipient::mail("ada@example.com"), &Notice)
        .await;
    assert!(result.is_err());
    assert!(*failed.lock().unwrap());
    Ok(())
}

#[tokio::test]
async fn synchronous_notification_events_expose_route_without_rendered_content() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut events = EventDispatcher::builder();
    let captured = Arc::clone(&seen);
    events.listen(move |event: Arc<NotificationSending>| {
        let captured = Arc::clone(&captured);
        async move {
            captured.lock().unwrap().push(("sending", event.0.recipient.clone(), event.0.channel));
            Ok::<_, Infallible>(())
        }
    });
    let captured = Arc::clone(&seen);
    events.listen(move |event: Arc<NotificationSent>| {
        let captured = Arc::clone(&captured);
        async move {
            captured.lock().unwrap().push(("sent", event.0.recipient.clone(), event.0.channel));
            Ok::<_, Infallible>(())
        }
    });
    NotificationDispatcher::new(Mailer::log())
        .with_events(events.build())
        .send(&AnonymousRecipient::mail("ada@example.com"), &Notice)
        .await
        .unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        vec![("sending", "ada@example.com".into(), Channel::Mail), ("sent", "ada@example.com".into(), Channel::Mail)]
    );
}

#[tokio::test]
async fn cancelled_mail_does_not_emit_notification_sent() {
    let mut mail_events = EventDispatcher::builder();
    mail_events.listen(|_: Arc<MessageSending>| async { Ok::<_, Infallible>(ListenerControl::Stop) });
    let sent = Arc::new(Mutex::new(false));
    let mut events = EventDispatcher::builder();
    let captured = Arc::clone(&sent);
    events.listen(move |_: Arc<NotificationSent>| {
        let captured = Arc::clone(&captured);
        async move {
            *captured.lock().unwrap() = true;
            Ok::<_, Infallible>(())
        }
    });
    NotificationDispatcher::new(Mailer::log().with_events(mail_events.build()))
        .with_events(events.build())
        .send(&AnonymousRecipient::mail("ada@example.com"), &Notice)
        .await
        .unwrap();
    assert!(!*sent.lock().unwrap());
}

#[tokio::test]
async fn stopping_notification_sending_prevents_queuing() -> Result<(), Box<dyn Error>> {
    use aurelin_framework::queue;
    let db = tempfile::NamedTempFile::new()?;
    let url = format!("sqlite://{}?mode=rwc", db.path().display());
    let pool = apalis_sqlite::SqlitePool::connect(&url).await?;
    apalis_sqlite::SqliteStorage::setup(&pool).await?;
    let mut queue = queue::sqlite::<MailMessage>(&url).await?;
    let mut events = EventDispatcher::builder();
    events.listen(|_: Arc<NotificationSending>| async { Ok::<_, Infallible>(ListenerControl::Stop) });
    NotificationDispatcher::new(Mailer::log())
        .with_events(events.build())
        .queue(&AnonymousRecipient::mail("ada@example.com"), &Notice, &mut queue)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Jobs").fetch_one(&pool).await?;
    assert_eq!(count, 0);
    Ok(())
}

#[tokio::test]
async fn enqueued_notification_is_not_reported_as_sent() -> Result<(), Box<dyn Error>> {
    use aurelin_framework::queue;
    let db = tempfile::NamedTempFile::new()?;
    let url = format!("sqlite://{}?mode=rwc", db.path().display());
    let pool = apalis_sqlite::SqlitePool::connect(&url).await?;
    apalis_sqlite::SqliteStorage::setup(&pool).await?;
    let mut queue = queue::sqlite::<MailMessage>(&url).await?;
    let sent = Arc::new(Mutex::new(false));
    let captured = Arc::clone(&sent);
    let mut events = EventDispatcher::builder();
    events.listen(move |_: Arc<NotificationSent>| {
        let captured = Arc::clone(&captured);
        async move {
            *captured.lock().unwrap() = true;
            Ok::<_, Infallible>(())
        }
    });
    NotificationDispatcher::new(Mailer::log())
        .with_events(events.build())
        .queue(&AnonymousRecipient::mail("ada@example.com"), &Notice, &mut queue)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Jobs").fetch_one(&pool).await?;
    assert_eq!(count, 1);
    assert!(!*sent.lock().unwrap());
    Ok(())
}

#[tokio::test]
async fn notification_listener_failure_is_returned_without_delivery() {
    use std::io;
    let mut events = EventDispatcher::builder();
    events.listen(|_: Arc<NotificationSending>| async { Err::<(), _>(io::Error::other("listener failed")) });
    let result = NotificationDispatcher::new(Mailer::log())
        .with_events(events.build())
        .send(&AnonymousRecipient::mail("ada@example.com"), &Notice)
        .await;
    assert!(matches!(result, Err(DispatchError::Event(_))));
}
