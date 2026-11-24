use std::{
    collections::HashMap,
    error::Error,
    io::{self, BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use aurelin_framework::{
    mail::{MailError, MailMessage, Mailer},
    notifications::{
        AnonymousRecipient, Channel, DispatchError, Notification, NotificationDispatcher, NotificationError,
    },
};

struct Invoice {
    selected: bool,
    calls: Arc<AtomicUsize>,
}

impl Notification<AnonymousRecipient> for Invoice {
    fn via(&self, _: &AnonymousRecipient) -> Result<Vec<Channel>, NotificationError> {
        Ok(if self.selected { vec![Channel::Mail] } else { vec![] })
    }

    fn to_mail(&self, _: &AnonymousRecipient) -> Result<MailMessage, NotificationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(MailMessage::new("Invoice paid", "Plain invoice").to("wrong@example.com")?.html("<p>HTML invoice</p>"))
    }
}

type SmtpServer = thread::JoinHandle<Result<(Vec<String>, String), String>>;

fn smtp_server() -> Result<(Mailer, SmtpServer), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
        stream.write_all(b"220 localhost ESMTP\r\n").map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut recipients = Vec::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("disconnected before DATA".into());
            }
            if line.starts_with("EHLO") {
                stream.write_all(b"250 localhost\r\n").map_err(|e| e.to_string())?;
            } else if line.starts_with("MAIL FROM:") || line.starts_with("RCPT TO:") {
                if line.starts_with("RCPT TO:") {
                    recipients.push(line);
                }
                stream.write_all(b"250 OK\r\n").map_err(|e| e.to_string())?;
            } else if line.starts_with("DATA") {
                stream.write_all(b"354 End data\r\n").map_err(|e| e.to_string())?;
                let mut message = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                        return Err("disconnected during DATA".into());
                    }
                    if line == ".\r\n" {
                        break;
                    }
                    message.push_str(&line);
                }
                stream.write_all(b"250 queued\r\n").map_err(|e| e.to_string())?;
                return Ok((recipients, message));
            } else {
                return Err(format!("unexpected SMTP command: {line}"));
            }
        }
    });
    let environment = HashMap::from([
        ("MAIL_MAILER".into(), "smtp".into()),
        ("MAIL_HOST".into(), "127.0.0.1".into()),
        ("MAIL_PORT".into(), port.to_string()),
        ("MAIL_ENCRYPTION".into(), "none".into()),
        ("MAIL_FROM_ADDRESS".into(), "default@example.com".into()),
    ]);
    Ok((Mailer::from_environment(&environment)?, server))
}

#[tokio::test]
async fn notification_routes_rendered_alternatives_through_real_smtp() -> Result<(), Box<dyn Error>> {
    let (mailer, server) = smtp_server()?;
    let notification = Invoice { selected: true, calls: Arc::new(AtomicUsize::new(0)) };
    NotificationDispatcher::new(mailer).send(&AnonymousRecipient::mail("ada@example.com"), &notification).await?;
    let (recipients, message) = server.join().map_err(|_| "server panicked")??;
    assert_eq!(recipients.len(), 1);
    assert!(recipients.iter().any(|recipient| recipient.contains("ada@example.com")));
    for expected in [
        "From: default@example.com",
        "To: ada@example.com",
        "Subject: Invoice paid",
        "multipart/alternative",
        "Plain invoice",
        "<p>HTML invoice</p>",
    ] {
        assert!(message.contains(expected), "missing {expected}: {message}");
    }
    Ok(())
}

#[tokio::test]
async fn no_selected_channels_do_not_compose_or_attempt_delivery() -> Result<(), Box<dyn Error>> {
    let calls = Arc::new(AtomicUsize::new(0));
    let notification = Invoice { selected: false, calls: calls.clone() };
    NotificationDispatcher::new(Mailer::log()).send(&AnonymousRecipient::mail("invalid"), &notification).await?;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn smtp_preserves_explicit_sender_and_copy_envelope_without_bcc_header() -> Result<(), Box<dyn Error>> {
    let (mailer, server) = smtp_server()?;
    let message = MailMessage::new("Update", "hello")
        .to("ada@example.com")?
        .from("other@example.com")?
        .reply_to("reply@example.com")?
        .cc("copy@example.com")?
        .bcc("hidden@example.com")?;
    mailer.send(message).await?;
    let (recipients, message) = server.join().map_err(|_| "server panicked")??;
    for address in ["ada@example.com", "copy@example.com", "hidden@example.com"] {
        assert!(recipients.iter().any(|recipient| recipient.contains(address)));
    }
    for expected in ["From: other@example.com", "Reply-To: reply@example.com", "Cc: copy@example.com"] {
        assert!(message.contains(expected), "missing {expected}: {message}");
    }
    assert!(!message.contains("hidden@example.com"));
    Ok(())
}

#[tokio::test]
async fn invalid_addresses_and_missing_recipients_fail_even_for_log_mail() -> Result<(), Box<dyn Error>> {
    assert!(MailMessage::new("subject", "body").to("not an address").is_err());
    assert!(Mailer::log().send(MailMessage::new("subject", "body")).await.is_err());
    Ok(())
}

#[tokio::test]
async fn queued_mail_is_dispatched_and_delivered_by_a_real_apalis_worker() -> Result<(), Box<dyn Error>> {
    use aurelin_framework::queue::{self, BoxDynError, TaskSink, WorkerBuilder, WorkerContext};
    let database = tempfile::NamedTempFile::new()?;
    let url = format!("sqlite://{}?mode=rwc", database.path().display());
    let pool = apalis_sqlite::SqlitePool::connect(&url).await?;
    apalis_sqlite::SqliteStorage::setup(&pool).await?;
    pool.close().await;
    let mut queue = queue::sqlite::<MailMessage>(&url).await?;
    queue.push(MailMessage::new("Queued invoice", "Delivered by worker").to("queued@example.com")?).await?;
    let (mailer, server) = smtp_server()?;
    let deliveries = Arc::new(AtomicUsize::new(0));
    let observed = deliveries.clone();
    WorkerBuilder::new("mail-worker")
        .backend(queue)
        .build(move |message: MailMessage, worker: WorkerContext| {
            let mailer = mailer.clone();
            let observed = observed.clone();
            async move {
                mailer.send(message).await?;
                observed.fetch_add(1, Ordering::SeqCst);
                worker.stop()?;
                Ok::<(), BoxDynError>(())
            }
        })
        .run()
        .await?;
    let (recipients, message) = server.join().map_err(|_| "server panicked")??;
    assert_eq!(deliveries.load(Ordering::SeqCst), 1);
    assert_eq!(recipients.len(), 1);
    assert!(recipients[0].contains("queued@example.com"));
    assert!(message.contains("Queued invoice"));
    assert!(message.contains("Delivered by worker"));
    Ok(())
}

#[test]
fn serialized_mail_revalidates_addresses_when_read_from_the_queue() -> Result<(), Box<dyn Error>> {
    let message = MailMessage::new("subject", "body").to("ada@example.com")?;
    let mut data = serde_json::to_value(message)?;
    data["recipients"][0] = "invalid address".into();
    assert!(serde_json::from_value::<MailMessage>(data).is_err());
    let mut data = serde_json::to_value(MailMessage::new("subject", "body").to("ada@example.com")?)?;
    data["recipients"][0] = serde_json::json!({"email": {"user": "invalid\n", "domain": "example.com"}});
    assert!(serde_json::from_value::<MailMessage>(data).is_err());
    Ok(())
}

#[tokio::test]
async fn smtp_connection_failure_is_returned_to_the_caller() -> Result<(), Box<dyn Error>> {
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
    let error = Mailer::from_environment(&environment)?
        .send(MailMessage::new("subject", "body").to("ada@example.com")?)
        .await
        .expect_err("SMTP failure must be returned");
    assert!(matches!(error, MailError::Delivery(_)));
    Ok(())
}

struct FailedNotification {
    selection: bool,
}

impl Notification<AnonymousRecipient> for FailedNotification {
    fn via(&self, _: &AnonymousRecipient) -> Result<Vec<Channel>, NotificationError> {
        if self.selection { Err(io::Error::other("selection failed").into()) } else { Ok(vec![Channel::Mail]) }
    }
    fn to_mail(&self, _: &AnonymousRecipient) -> Result<MailMessage, NotificationError> {
        Err(io::Error::other("composition failed").into())
    }
}

#[tokio::test]
async fn selection_and_composition_errors_prevent_delivery() {
    let dispatcher = NotificationDispatcher::new(Mailer::log());
    let recipient = AnonymousRecipient::mail("ada@example.com");
    let error =
        dispatcher.send(&recipient, &FailedNotification { selection: true }).await.expect_err("selection must fail");
    assert!(matches!(error, DispatchError::Selection(_)));
    let error =
        dispatcher.send(&recipient, &FailedNotification { selection: false }).await.expect_err("composition must fail");
    assert!(matches!(error, DispatchError::Composition(_)));
}

#[tokio::test]
async fn cloned_dispatchers_send_concurrently_without_sharing_recipient_state() -> Result<(), Box<dyn Error>> {
    let dispatcher = NotificationDispatcher::new(Mailer::log());
    let notification = Arc::new(Invoice { selected: true, calls: Arc::new(AtomicUsize::new(0)) });
    let mut tasks = Vec::new();
    for i in 0..16 {
        let dispatcher = dispatcher.clone();
        let notification = notification.clone();
        tasks.push(tokio::spawn(async move {
            dispatcher.send(&AnonymousRecipient::mail(format!("user{i}@example.com")), &*notification).await
        }));
    }
    for task in tasks {
        task.await??;
    }
    assert_eq!(notification.calls.load(Ordering::SeqCst), 16);
    Ok(())
}
