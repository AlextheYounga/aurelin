use std::{collections::HashMap, num::ParseIntError};

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    address::AddressError,
    error::Error as LettreError,
    message::Mailbox,
    transport::smtp::{Error as SmtpError, authentication::Credentials},
};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Option<Mailbox>,
}

impl Mailer {
    pub fn log() -> Self {
        Self { transport: None, from: None }
    }

    pub fn from_environment(environment: &HashMap<String, String>) -> Result<Self, MailError> {
        match environment.get("MAIL_MAILER").map_or("log", String::as_str) {
            "log" => Ok(Self::log()),
            "smtp" => Self::smtp(environment),
            mailer => Err(MailError::UnsupportedMailer(mailer.to_owned())),
        }
    }

    pub async fn send(&self, recipient: &str, subject: &str, body: String) -> Result<(), MailError> {
        let Some(transport) = &self.transport else {
            tracing::info!(%recipient, %subject, %body, "mail notification");
            return Ok(());
        };
        let message = Message::builder()
            .from(self.from.clone().ok_or(MailError::Missing("MAIL_FROM_ADDRESS"))?)
            .to(recipient.parse()?)
            .subject(subject)
            .body(body)?;
        transport.send(message).await?;
        Ok(())
    }

    fn smtp(environment: &HashMap<String, String>) -> Result<Self, MailError> {
        let host = required(environment, "MAIL_HOST")?;
        let encryption = environment.get("MAIL_ENCRYPTION").map_or("tls", String::as_str);
        let mut builder = match encryption {
            "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)?,
            "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
            "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
            value => return Err(MailError::UnsupportedEncryption(value.to_owned())),
        };
        if let Some(port) = environment.get("MAIL_PORT").filter(|value| !value.is_empty()) {
            builder = builder.port(port.parse()?);
        }
        match (
            environment.get("MAIL_USERNAME").filter(|value| !value.is_empty()),
            environment.get("MAIL_PASSWORD").filter(|value| !value.is_empty()),
        ) {
            (Some(username), Some(password)) => {
                builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
            }
            (None, None) => {}
            _ => return Err(MailError::IncompleteCredentials),
        }
        let address = required(environment, "MAIL_FROM_ADDRESS")?.parse()?;
        let from = Mailbox::new(environment.get("MAIL_FROM_NAME").cloned(), address);
        Ok(Self { transport: Some(builder.build()), from: Some(from) })
    }
}

fn required<'a>(environment: &'a HashMap<String, String>, name: &'static str) -> Result<&'a str, MailError> {
    environment.get(name).filter(|value| !value.is_empty()).map(String::as_str).ok_or(MailError::Missing(name))
}

#[derive(Debug, Error)]
pub enum MailError {
    #[error("{0} must be set for the configured mailer")]
    Missing(&'static str),
    #[error("unsupported mailer: {0}")]
    UnsupportedMailer(String),
    #[error("unsupported mail encryption: {0}")]
    UnsupportedEncryption(String),
    #[error("MAIL_USERNAME and MAIL_PASSWORD must either both be set or both be omitted")]
    IncompleteCredentials,
    #[error("invalid mail port")]
    Port(#[from] ParseIntError),
    #[error("invalid mail address")]
    Address(#[from] AddressError),
    #[error("could not build the mail message")]
    Message(#[from] LettreError),
    #[error("SMTP delivery failed")]
    Delivery(#[from] SmtpError),
}
