use std::str::FromStr;

use lettre::{
    Message,
    address::AddressError,
    message::{Mailbox, MultiPart, SinglePart},
};

use super::MailError;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
struct Address(Mailbox);

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

impl TryFrom<String> for Address {
    type Error = AddressError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Address> for String {
    fn from(address: Address) -> Self {
        address.0.to_string()
    }
}

/// An application-rendered email, independent of its delivery transport.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct MailMessage {
    recipients: Vec<Address>,
    subject: String,
    text: String,
    html: Option<String>,
    from: Option<Address>,
    reply_to: Vec<Address>,
    cc: Vec<Address>,
    bcc: Vec<Address>,
}

impl MailMessage {
    /// Create a message with an explicit subject and rendered plain-text body.
    pub fn new(subject: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            recipients: Vec::new(),
            subject: subject.into(),
            text: text.into(),
            html: None,
            from: None,
            reply_to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
        }
    }

    /// Add a recipient, validating its address.
    pub fn to(mut self, address: &str) -> Result<Self, MailError> {
        self.recipients.push(address.parse()?);
        Ok(self)
    }

    /// Override the configured sender.
    pub fn from(mut self, address: &str) -> Result<Self, MailError> {
        self.from = Some(address.parse()?);
        Ok(self)
    }

    /// Add a reply-to address.
    pub fn reply_to(mut self, address: &str) -> Result<Self, MailError> {
        self.reply_to.push(address.parse()?);
        Ok(self)
    }

    /// Add a carbon-copy recipient.
    pub fn cc(mut self, address: &str) -> Result<Self, MailError> {
        self.cc.push(address.parse()?);
        Ok(self)
    }

    /// Add a blind carbon-copy recipient.
    pub fn bcc(mut self, address: &str) -> Result<Self, MailError> {
        self.bcc.push(address.parse()?);
        Ok(self)
    }

    /// Add an application-rendered HTML alternative.
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html = Some(html.into());
        self
    }

    pub(crate) fn routed_to(mut self, address: &str) -> Result<Self, MailError> {
        self.recipients = vec![address.parse()?];
        Ok(self)
    }

    pub(super) fn build(self, default_from: Option<Mailbox>) -> Result<Message, MailError> {
        let from =
            self.from.map(|address| address.0).or(default_from).ok_or(MailError::Missing("MAIL_FROM_ADDRESS"))?;
        let mut builder = Message::builder().from(from).subject(self.subject);
        for address in self.recipients {
            builder = builder.to(address.0);
        }
        for address in self.reply_to {
            builder = builder.reply_to(address.0);
        }
        for address in self.cc {
            builder = builder.cc(address.0);
        }
        for address in self.bcc {
            builder = builder.bcc(address.0);
        }
        Ok(match self.html {
            Some(html) => builder.multipart(MultiPart::alternative_plain_html(self.text, html))?,
            None => builder.singlepart(SinglePart::plain(self.text))?,
        })
    }
}
