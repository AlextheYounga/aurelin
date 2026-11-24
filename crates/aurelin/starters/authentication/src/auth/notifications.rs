use super::{AuthBackend, AuthError, User, normalize_email};
use aurelin_framework::mail::MailMessage;

impl AuthBackend {
    pub async fn send_email_verification(&self, user: &User) -> Result<(), AuthError> {
        let url = format!(
            "{}{}",
            self.application_url,
            self.email_verification_url(user)?
        );
        self.mailer
            .send(MailMessage::new(
                "Verify your email address",
                format!(
                    "Please verify your email address by visiting:\n\n{url}\n\nIf you did not create an account, no further action is required."
                ),
            ).to(&user.email)?)
            .await?;
        Ok(())
    }

    pub async fn send_password_reset(&self, email: &str, token: &str) -> Result<(), AuthError> {
        let email = normalize_email(email);
        let url = format!(
            "{}/reset-password/{token}?email={}",
            self.application_url,
            urlencoding::encode(&email)
        );
        self.mailer
            .send(MailMessage::new(
                "Reset your password",
                format!(
                    "We received a password reset request for your account. Reset your password by visiting:\n\n{url}\n\nThis link expires in 60 minutes. If you did not request a password reset, no further action is required."
                ),
            ).to(&email)?)
            .await?;
        Ok(())
    }
}
