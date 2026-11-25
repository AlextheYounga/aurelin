//! Recipient-aware notification dispatch with an explicit mail channel.

use std::{any::type_name, error::Error, fmt};

use crate::events::{DispatchError as EventError, DispatchOutcome, EventDispatcher};
use crate::mail::{MailError, MailMessage, Mailer, SendOutcome};
use crate::queue::TaskSink;
use apalis_sqlite::SqliteStorage;

/// An application error encountered while selecting or composing a notification.
pub type NotificationError = Box<dyn Error + Send + Sync + 'static>;

/// A supported notification delivery channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Channel {
    /// Deliver using the configured mailer.
    Mail,
}

/// Non-content metadata for a notification channel delivery.
#[derive(Clone, Debug)]
pub struct NotificationMetadata {
    /// The Rust notification type selected by the application.
    pub notification_type: &'static str,
    /// The selected channel.
    pub channel: Channel,
    /// The destination address; treat it as personal data.
    pub recipient: String,
}

/// Before a prepared notification is sent or enqueued. Stop cancels the channel.
pub struct NotificationSending(pub NotificationMetadata);
/// A synchronous notification was accepted by its mail transport.
pub struct NotificationSent(pub NotificationMetadata);
/// A synchronous notification's mail delivery failed.
pub struct NotificationFailed(pub NotificationMetadata);

/// Application-owned notification content and channel selection.
pub trait Notification<Recipient> {
    /// Select channels separately for each recipient.
    fn via(&self, recipient: &Recipient) -> Result<Vec<Channel>, NotificationError>;

    /// Compose the application-rendered mail for this recipient.
    fn to_mail(&self, recipient: &Recipient) -> Result<MailMessage, NotificationError>;
}

/// Explicit routing for mail notifications, independent of any user model.
pub trait MailRecipient<N> {
    /// Return the address to use for this notification, or no route.
    fn route_mail(&self, notification: &N) -> Result<Option<String>, NotificationError>;
}

/// An on-demand recipient with an explicit mail address.
#[derive(Clone, Debug)]
pub struct AnonymousRecipient {
    address: String,
}

impl AnonymousRecipient {
    /// Create an on-demand mail recipient.
    pub fn mail(address: impl Into<String>) -> Self {
        Self { address: address.into() }
    }
}

impl<N> MailRecipient<N> for AnonymousRecipient {
    fn route_mail(&self, _notification: &N) -> Result<Option<String>, NotificationError> {
        Ok(Some(self.address.clone()))
    }
}

/// Dispatches notifications through explicitly supplied channel dependencies.
#[derive(Clone)]
pub struct NotificationDispatcher {
    mailer: Mailer,
    events: Option<EventDispatcher>,
}

impl fmt::Debug for NotificationDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationDispatcher").finish_non_exhaustive()
    }
}

impl NotificationDispatcher {
    /// Create a dispatcher using the given mailer.
    pub fn new(mailer: Mailer) -> Self {
        Self { mailer, events: None }
    }

    /// Configure notification lifecycle listeners explicitly.
    pub fn with_events(mut self, events: EventDispatcher) -> Self {
        self.events = Some(events);
        self
    }

    /// Deliver selected channels in order. An empty selection does nothing.
    pub async fn send<N, R>(&self, recipient: &R, notification: &N) -> Result<(), DispatchError>
    where
        N: Notification<R>,
        R: MailRecipient<N>,
    {
        let channels = notification.via(recipient).map_err(DispatchError::Selection)?;
        for channel in channels {
            match channel {
                Channel::Mail => {
                    if let Some((message, address)) = mail_message(recipient, notification)? {
                        let metadata =
                            NotificationMetadata { notification_type: type_name::<N>(), channel, recipient: address };
                        if !self.sending(&metadata).await? {
                            continue;
                        }
                        match self.mailer.send_with_outcome(message).await {
                            Ok(SendOutcome::Sent) => {
                                if let Some(events) = &self.events {
                                    events.dispatch(NotificationSent(metadata)).await?;
                                }
                            }
                            Ok(SendOutcome::Cancelled) => {}
                            Err(error) => {
                                if let Some(events) = &self.events {
                                    events.dispatch(NotificationFailed(metadata)).await?;
                                }
                                return Err(error.into());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Render recipient-specific mail and enqueue it on the existing mail queue.
    ///
    /// Content and addresses are persisted in the queue. Applications must
    /// protect queue storage and register a worker that sends `MailMessage` jobs.
    pub async fn queue<N, R>(
        &self,
        recipient: &R,
        notification: &N,
        queue: &mut SqliteStorage<MailMessage>,
    ) -> Result<(), DispatchError>
    where
        N: Notification<R>,
        R: MailRecipient<N>,
    {
        for channel in notification.via(recipient).map_err(DispatchError::Selection)? {
            match channel {
                Channel::Mail => {
                    if let Some((message, address)) = mail_message(recipient, notification)? {
                        let metadata =
                            NotificationMetadata { notification_type: type_name::<N>(), channel, recipient: address };
                        if !self.sending(&metadata).await? {
                            continue;
                        }
                        queue.push(message).await.map_err(|error| DispatchError::Queue(Box::new(error)))?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn sending(&self, metadata: &NotificationMetadata) -> Result<bool, DispatchError> {
        match &self.events {
            Some(events) => {
                Ok(events.dispatch(NotificationSending(metadata.clone())).await? != DispatchOutcome::Stopped)
            }
            None => Ok(true),
        }
    }
}

fn mail_message<N, R>(recipient: &R, notification: &N) -> Result<Option<(MailMessage, String)>, DispatchError>
where
    N: Notification<R>,
    R: MailRecipient<N>,
{
    let message = notification.to_mail(recipient).map_err(DispatchError::Composition)?;
    let Some(address) = recipient.route_mail(notification).map_err(DispatchError::Routing)? else {
        return Ok(None);
    };
    Ok(Some((message.routed_to(&address)?, address)))
}

/// Failure to select, compose, route, or deliver a notification.
#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    /// Channel selection failed.
    #[error("notification channel selection failed")]
    Selection(#[source] NotificationError),
    /// Notification composition failed.
    #[error("notification mail composition failed")]
    Composition(#[source] NotificationError),
    /// Recipient routing failed.
    #[error("notification mail routing failed")]
    Routing(#[source] NotificationError),
    /// Message construction or transport failed.
    #[error("notification mail delivery failed")]
    Mail(#[from] MailError),
    /// The rendered notification could not be enqueued.
    #[error("notification queue dispatch failed")]
    Queue(#[source] Box<dyn Error + Send + Sync>),
    /// A lifecycle listener failed.
    #[error(transparent)]
    Event(#[from] EventError),
}
