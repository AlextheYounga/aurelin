use aurelin_fortify::axum::{
    CreateUserOutcome, EmailVerificationRequest, FortifyBackend, FortifyUser, PasskeyLoginOutcome,
    PasskeyRegistrationOutcome, PasskeyRegistrationRequest,
};
use aurelin_fortify::passkeys::PasskeyCeremony;
use aurelin_framework::auth::{PasswordResetRequest, RateLimitOutcome};
use serde_json::Value;

use super::{AuthBackend, AuthError, PublicUser, User};
use crate::actions::CreateUser;

impl FortifyUser for User {
    type Public = PublicUser;

    fn email(&self) -> &str {
        &self.email
    }

    fn email_is_verified(&self) -> bool {
        self.email_verified_at.is_some()
    }

    fn two_factor_is_enabled(&self) -> bool {
        self.two_factor_secret.is_some() && self.two_factor_confirmed_at.is_some()
    }

    fn public(&self) -> Self::Public {
        PublicUser::from(self)
    }
}

impl FortifyBackend for AuthBackend {
    fn authentication_timebox_duration(&self) -> std::time::Duration {
        self.authentication_timebox_duration
    }

    fn password_rules(&self) -> &'static str {
        AuthBackend::password_rules(self)
    }

    fn application_url(&self) -> &str {
        AuthBackend::application_url(self)
    }

    async fn validate_password(&self, password: &str) -> Result<(), String> {
        AuthBackend::validate_password(self, password)
            .await
            .map_err(|violation| violation.message())
    }

    async fn login_retry_after(&self, key: &str) -> Option<std::time::Duration> {
        AuthBackend::login_retry_after(self, key).await
    }

    async fn record_failed_login(&self, key: &str) -> RateLimitOutcome {
        AuthBackend::record_failed_login(self, key).await
    }

    async fn clear_failed_logins(&self, key: &str) {
        AuthBackend::clear_failed_logins(self, key).await;
    }

    async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> Result<CreateUserOutcome<Self::User>, Self::Error> {
        Ok(
            match AuthBackend::create_user(self, name, email, password).await? {
                CreateUser::Created(user) => CreateUserOutcome::Created(user),
                CreateUser::EmailAlreadyExists => CreateUserOutcome::EmailAlreadyExists,
            },
        )
    }

    async fn send_email_verification(&self, user: &User) -> Result<(), AuthError> {
        AuthBackend::send_email_verification(self, user).await
    }

    async fn create_password_reset_token(
        &self,
        email: &str,
    ) -> Result<PasswordResetRequest, AuthError> {
        AuthBackend::create_password_reset_token(self, email).await
    }

    async fn send_password_reset(&self, email: &str, token: &str) -> Result<(), AuthError> {
        AuthBackend::send_password_reset(self, email, token).await
    }

    async fn reset_password(
        &self,
        email: &str,
        token: &str,
        password: &str,
    ) -> Result<bool, AuthError> {
        AuthBackend::reset_password(self, email, token, password).await
    }

    async fn email_verification_attempt(&self, user_id: i64) -> RateLimitOutcome {
        AuthBackend::email_verification_attempt(self, user_id).await
    }

    async fn verify_email(
        &self,
        request: EmailVerificationRequest<'_, User>,
    ) -> Result<bool, AuthError> {
        AuthBackend::verify_email(
            self,
            request.user,
            request.email_hash,
            request.expires,
            request.signature,
        )
        .await
    }

    async fn two_factor_attempt(&self, user_id: i64) -> RateLimitOutcome {
        AuthBackend::two_factor_attempt(self, user_id).await
    }

    async fn challenged_user(&self, user_id: i64) -> Result<Option<User>, AuthError> {
        AuthBackend::challenged_user(self, user_id).await
    }

    async fn challenge_two_factor(
        &self,
        user: &User,
        code: Option<&str>,
        recovery_code: Option<&str>,
    ) -> Result<bool, AuthError> {
        AuthBackend::challenge_two_factor(self, user, code, recovery_code).await
    }

    async fn enable_two_factor(&self, user: &User) -> Result<User, AuthError> {
        AuthBackend::enable_two_factor(self, user).await
    }

    async fn confirm_two_factor(&self, user: &User, code: &str) -> Result<bool, AuthError> {
        AuthBackend::confirm_two_factor(self, user, code).await
    }

    async fn disable_two_factor(&self, user: &User) -> Result<(), AuthError> {
        AuthBackend::disable_two_factor(self, user).await
    }

    fn two_factor_qr_code(&self, user: &User) -> Result<Option<String>, AuthError> {
        AuthBackend::two_factor_qr_code(self, user)
    }

    fn two_factor_secret_key(&self, user: &User) -> Result<Option<String>, AuthError> {
        AuthBackend::two_factor_secret_key(self, user)
    }

    fn two_factor_recovery_codes(&self, user: &User) -> Result<Vec<String>, AuthError> {
        AuthBackend::two_factor_recovery_codes(self, user)
    }

    async fn regenerate_two_factor_recovery_codes(&self, user: &User) -> Result<(), AuthError> {
        AuthBackend::regenerate_two_factor_recovery_codes(self, user).await
    }

    async fn passkey_attempt(&self, key: &str) -> RateLimitOutcome {
        AuthBackend::passkey_attempt(self, key).await
    }

    async fn start_passkey_registration(&self, user: &User) -> Result<PasskeyCeremony, AuthError> {
        AuthBackend::start_passkey_registration(self, user).await
    }

    async fn finish_passkey_registration(
        &self,
        request: PasskeyRegistrationRequest<'_, User>,
    ) -> Result<PasskeyRegistrationOutcome, AuthError> {
        match AuthBackend::finish_passkey_registration(
            self,
            request.ceremony_id,
            request.user,
            request.name,
            request.credential,
        )
        .await
        {
            Ok(passkey) => Ok(PasskeyRegistrationOutcome::Registered {
                id: passkey.id.to_string(),
                name: passkey.name,
            }),
            Err(AuthError::PasskeyAlreadyRegistered) => {
                Ok(PasskeyRegistrationOutcome::AlreadyRegistered)
            }
            Err(AuthError::Passkey(_)) => Ok(PasskeyRegistrationOutcome::Invalid),
            Err(error) => Err(error),
        }
    }

    fn start_passkey_login(&self) -> Result<PasskeyCeremony, AuthError> {
        AuthBackend::start_passkey_login(self)
    }

    async fn finish_passkey_login(
        &self,
        ceremony_id: &str,
        credential: Value,
    ) -> Result<PasskeyLoginOutcome<User>, AuthError> {
        match AuthBackend::finish_passkey_login(self, ceremony_id, credential).await {
            Ok(Some(user)) => Ok(PasskeyLoginOutcome::Authenticated(user)),
            Ok(None) | Err(AuthError::Passkey(_)) => Ok(PasskeyLoginOutcome::Invalid),
            Err(error) => Err(error),
        }
    }

    async fn start_passkey_confirmation(&self, user: &User) -> Result<PasskeyCeremony, AuthError> {
        AuthBackend::start_passkey_confirmation(self, user).await
    }

    async fn finish_passkey_confirmation(
        &self,
        ceremony_id: &str,
        user: &User,
        credential: Value,
    ) -> Result<bool, AuthError> {
        match AuthBackend::finish_passkey_confirmation(self, ceremony_id, user, credential).await {
            Ok(confirmed) => Ok(confirmed),
            Err(AuthError::Passkey(_)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn delete_passkey(&self, user_id: i64, passkey_id: i64) -> Result<bool, AuthError> {
        AuthBackend::delete_passkey(self, user_id, passkey_id).await
    }
}
