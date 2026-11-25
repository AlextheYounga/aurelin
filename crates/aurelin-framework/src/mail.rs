use std::{collections::HashMap, fmt, num::ParseIntError};

use crate::events::{DispatchError, DispatchOutcome, EventDispatcher};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    address::AddressError,
    error::Error as LettreError,
    message::Mailbox,
    transport::smtp::{Error as SmtpError, authentication::Credentials},
};
use thiserror::Error;

mod message;
pub use message::MailMessage;

/// Emitted after message construction, before transport delivery. Stopping
/// propagation cancels delivery.
pub struct MessageSending(pub MailMessage);

/// Emitted after successful transport delivery.
pub struct MessageSent(pub MailMessage);

/// Emitted when message construction or transport delivery fails.
pub struct MessageFailed(pub MailMessage);

/// Whether the transport accepted a message or a listener cancelled it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendOutcome {
    /// The transport accepted the message.
    Sent,
    /// A sending listener cancelled delivery.
    Cancelled,
}

#[derive(Clone)]
pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Option<Mailbox>,
    events: Option<EventDispatcher>,
}

impl fmt::Debug for Mailer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Mailer").field("smtp", &self.transport.is_some()).finish_non_exhaustive()
    }
}

impl Mailer {
    pub fn log() -> Self {
        Self { transport: None, from: None, events: None }
    }

    /// Use the application's dispatcher for mail lifecycle events.
    pub fn with_events(mut self, events: EventDispatcher) -> Self {
        self.events = Some(events);
        self
    }

    pub fn from_environment(environment: &HashMap<String, String>) -> Result<Self, MailError> {
        match environment.get("MAIL_MAILER").map_or("log", String::as_str) {
            "log" => {
                let from = environment
                    .get("MAIL_FROM_ADDRESS")
                    .filter(|value| !value.is_empty())
                    .map(|address| {
                        address.parse().map(|address| Mailbox::new(environment.get("MAIL_FROM_NAME").cloned(), address))
                    })
                    .transpose()?;
                Ok(Self { transport: None, from, events: None })
            }
            "smtp" => Self::smtp(environment),
            mailer => Err(MailError::UnsupportedMailer(mailer.to_owned())),
        }
    }

    pub async fn send(&self, message: MailMessage) -> Result<(), MailError> {
        self.send_with_outcome(message).await.map(|_| ())
    }

    /// Send mail while distinguishing cancellation from transport acceptance.
    pub async fn send_with_outcome(&self, message: MailMessage) -> Result<SendOutcome, MailError> {
        let from = match (&self.from, &self.transport) {
            (None, None) => Some(Mailbox::new(None, "hello@example.com".parse()?)),
            _ => self.from.clone(),
        };
        let built = match message.clone().build(from) {
            Ok(built) => built,
            Err(error) => {
                self.failed(message).await?;
                return Err(error);
            }
        };
        if let Some(events) = &self.events {
            if events.dispatch(MessageSending(message.clone())).await? == DispatchOutcome::Stopped {
                return Ok(SendOutcome::Cancelled);
            }
        }
        if let Some(transport) = &self.transport {
            if let Err(error) = transport.send(built).await {
                self.failed(message).await?;
                return Err(error.into());
            }
        } else {
            tracing::info!(message = %String::from_utf8_lossy(&built.formatted()), "mail notification");
        }
        if let Some(events) = &self.events {
            events.dispatch(MessageSent(message)).await?;
        }
        Ok(SendOutcome::Sent)
    }

    async fn failed(&self, message: MailMessage) -> Result<(), MailError> {
        if let Some(events) = &self.events {
            events.dispatch(MessageFailed(message)).await?;
        }
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
        Ok(Self { transport: Some(builder.build()), from: Some(from), events: None })
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
    /// A lifecycle listener failed.
    #[error(transparent)]
    Event(#[from] DispatchError),
}
