use std::{
    collections::HashMap,
    error::Error,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

use __AURELIN_CRATE__::mail::Mailer;

#[tokio::test]
async fn smtp_mailer_delivers_a_message_to_the_configured_server() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let (message_sender, message_receiver) = mpsc::channel();
    let server = thread::spawn(move || -> Result<(), String> {
        let (stream, _) = listener.accept().map_err(|error| error.to_string())?;
        receive_message(stream, message_sender)
    });

    let environment = HashMap::from([
        ("MAIL_MAILER".into(), "smtp".into()),
        ("MAIL_HOST".into(), "127.0.0.1".into()),
        ("MAIL_PORT".into(), port.to_string()),
        ("MAIL_ENCRYPTION".into(), "none".into()),
        ("MAIL_FROM_ADDRESS".into(), "hello@example.com".into()),
        ("MAIL_FROM_NAME".into(), "Aurelin".into()),
    ]);
    let mailer = Mailer::from_environment(&environment)?;
    mailer
        .send(
            "ada@example.com",
            "Verify your email address",
            "Visit https://example.com/email/verify/signed".into(),
        )
        .await?;

    let message = message_receiver.recv_timeout(Duration::from_secs(5))?;
    assert!(message.contains("From: Aurelin <hello@example.com>"));
    assert!(message.contains("To: ada@example.com"));
    assert!(message.contains("Subject: Verify your email address"));
    assert!(message.contains("https://example.com/email/verify/signed"));
    server.join().map_err(|_| "SMTP test server panicked")??;
    Ok(())
}

fn receive_message(
    mut stream: TcpStream,
    message_sender: mpsc::Sender<String>,
) -> Result<(), String> {
    stream
        .write_all(b"220 localhost ESMTP\r\n")
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
    loop {
        let mut line = String::new();
        if reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return Err("SMTP client disconnected before DATA".into());
        }
        if line.starts_with("EHLO") {
            stream
                .write_all(b"250 localhost\r\n")
                .map_err(|error| error.to_string())?;
        } else if line.starts_with("MAIL FROM:") || line.starts_with("RCPT TO:") {
            stream
                .write_all(b"250 OK\r\n")
                .map_err(|error| error.to_string())?;
        } else if line.starts_with("DATA") {
            stream
                .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
                .map_err(|error| error.to_string())?;
            let mut message = String::new();
            loop {
                let mut message_line = String::new();
                reader
                    .read_line(&mut message_line)
                    .map_err(|error| error.to_string())?;
                if message_line == ".\r\n" {
                    break;
                }
                message.push_str(&message_line);
            }
            message_sender
                .send(message)
                .map_err(|error| error.to_string())?;
            stream
                .write_all(b"250 queued\r\n")
                .map_err(|error| error.to_string())?;
            return Ok(());
        } else {
            return Err(format!("unexpected SMTP command: {line}"));
        }
    }
}
