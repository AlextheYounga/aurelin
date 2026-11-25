use std::{
    convert::Infallible,
    io,
    sync::{Arc, Mutex},
};

use aurelin_framework::{
    events::{EventDispatcher, ListenerControl},
    mail::{MailError, MailMessage, Mailer, MessageFailed, MessageSending, MessageSent},
};

#[tokio::test]
async fn mail_lifecycle_events_follow_success_and_delivery_failure() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut events = EventDispatcher::builder();
    let sending = Arc::clone(&calls);
    events.listen(move |_: Arc<MessageSending>| {
        let calls = Arc::clone(&sending);
        async move {
            calls.lock().unwrap().push("sending");
            Ok::<_, Infallible>(())
        }
    });
    let sent = Arc::clone(&calls);
    events.listen(move |_: Arc<MessageSent>| {
        let calls = Arc::clone(&sent);
        async move {
            calls.lock().unwrap().push("sent");
            Ok::<_, Infallible>(())
        }
    });
    let failed = Arc::clone(&calls);
    events.listen(move |_: Arc<MessageFailed>| {
        let calls = Arc::clone(&failed);
        async move {
            calls.lock().unwrap().push("failed");
            Ok::<_, Infallible>(())
        }
    });
    let mailer = Mailer::log().with_events(events.build());
    mailer.send(MailMessage::new("subject", "body").to("ada@example.com").unwrap()).await.unwrap();
    assert!(mailer.send(MailMessage::new("subject", "missing recipient")).await.is_err());
    assert_eq!(*calls.lock().unwrap(), vec!["sending", "sent", "failed"]);
}

#[tokio::test]
async fn sending_listener_can_cancel_delivery_without_sent_or_failed_event() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut events = EventDispatcher::builder();
    events.listen(|_: Arc<MessageSending>| async { Ok::<_, Infallible>(ListenerControl::Stop) });
    let sent = Arc::clone(&calls);
    events.listen(move |_: Arc<MessageSent>| {
        let calls = Arc::clone(&sent);
        async move {
            calls.lock().unwrap().push("sent");
            Ok::<_, Infallible>(())
        }
    });
    let failed = Arc::clone(&calls);
    events.listen(move |_: Arc<MessageFailed>| {
        let calls = Arc::clone(&failed);
        async move {
            calls.lock().unwrap().push("failed");
            Ok::<_, Infallible>(())
        }
    });
    Mailer::log()
        .with_events(events.build())
        .send(MailMessage::new("subject", "body").to("ada@example.com").unwrap())
        .await
        .unwrap();
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failing_sending_listener_aborts_delivery_without_emitting_transport_failure() {
    let failures = Arc::new(Mutex::new(0));
    let mut events = EventDispatcher::builder();
    events.listen(|_: Arc<MessageSending>| async { Err::<(), _>(io::Error::other("listener failure")) });
    let observed = Arc::clone(&failures);
    events.listen(move |_: Arc<MessageFailed>| {
        let observed = Arc::clone(&observed);
        async move {
            *observed.lock().unwrap() += 1;
            Ok::<_, Infallible>(())
        }
    });
    let result = Mailer::log()
        .with_events(events.build())
        .send(MailMessage::new("subject", "body").to("ada@example.com").unwrap())
        .await;
    assert!(matches!(result, Err(MailError::Event(_))));
    assert_eq!(*failures.lock().unwrap(), 0);
}
