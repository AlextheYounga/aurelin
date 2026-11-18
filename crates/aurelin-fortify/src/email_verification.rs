use std::time::Duration;

use aurelin_framework::{
    auth::digest_token,
    signed_url::{SignedUrlError, TemporarySignedUrl},
};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct EmailVerification {
    signed_urls: TemporarySignedUrl,
    lifetime_seconds: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct EmailVerificationRequest<'a> {
    pub user_id: i64,
    pub email: &'a str,
    pub email_hash: &'a str,
    pub expires: i64,
    pub signature: &'a str,
}

impl EmailVerification {
    pub fn new(application_key: &[u8]) -> Result<Self, EmailVerificationError> {
        Ok(Self {
            signed_urls: TemporarySignedUrl::new(application_key)?,
            lifetime_seconds: Duration::from_secs(60 * 60).as_secs().cast_signed(),
        })
    }

    pub fn url(&self, user_id: i64, email: &str, now: i64) -> Result<String, EmailVerificationError> {
        let email_hash = digest_token(email);
        let expires = now + self.lifetime_seconds;
        let unsigned = format!("/email/verify/{user_id}/{email_hash}?expires={expires}");
        let signature = self.signed_urls.sign(&unsigned)?;
        Ok(format!("{unsigned}&signature={signature}"))
    }

    pub fn verify(&self, request: EmailVerificationRequest<'_>, now: i64) -> Result<bool, EmailVerificationError> {
        if request.expires < now || request.email_hash != digest_token(request.email) {
            return Ok(false);
        }
        let unsigned = format!("/email/verify/{}/{}?expires={}", request.user_id, request.email_hash, request.expires);
        Ok(self.signed_urls.verify(&unsigned, request.signature)?)
    }
}

#[derive(Debug, Error)]
pub enum EmailVerificationError {
    #[error(transparent)]
    SignedUrl(#[from] SignedUrlError),
}
