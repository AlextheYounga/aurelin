//! Recipient-aware notification dispatch with an explicit mail channel.

use std::error::Error;

use crate::mail::{MailError, MailMessage, Mailer};

/// An application error encountered while selecting or composing a notification.
pub type NotificationError = Box<dyn Error + Send + Sync + 'static>;

/// A supported notification delivery channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Channel {
    /// Deliver using the configured mailer.
    Mail,
}

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
#[derive(Clone, Debug)]
pub struct NotificationDispatcher {
    mailer: Mailer,
}

impl NotificationDispatcher {
    /// Create a dispatcher using the given mailer.
    pub fn new(mailer: Mailer) -> Self {
        Self { mailer }
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
                    let message = notification.to_mail(recipient).map_err(DispatchError::Composition)?;
                    let Some(address) = recipient.route_mail(notification).map_err(DispatchError::Routing)? else {
                        continue;
                    };
                    self.mailer.send(message.routed_to(&address)?).await?;
                }
            }
        }
        Ok(())
    }
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
}
