use std::error::Error;

use aurelin_framework::auth::{
    PasswordResetBroker, PasswordResetCredentials, PasswordResetError, PasswordResetRepository,
};
use thiserror::Error;

use crate::{ResetPasswordInput, ResetsUserPasswords};

#[derive(Clone, Debug)]
pub struct ResetPasswords<R, A> {
    broker: PasswordResetBroker<R>,
    action: A,
}

impl<R, A> ResetPasswords<R, A>
where
    R: PasswordResetRepository,
    A: ResetsUserPasswords,
{
    pub fn new(repository: R, action: A) -> Self {
        Self { broker: PasswordResetBroker::new(repository), action }
    }

    pub async fn reset(
        &self,
        credentials: PasswordResetCredentials<'_>,
        now: i64,
    ) -> Result<bool, ResetPasswordsError<R::Error, A::Error>> {
        let Some(reset) = self.broker.validate_reset(credentials, now).await.map_err(ResetPasswordsError::Broker)?
        else {
            return Ok(false);
        };

        self.action
            .reset(ResetPasswordInput { email: reset.email.to_owned(), password: reset.password.to_owned() })
            .await
            .map_err(ResetPasswordsError::Action)?;

        self.broker.complete_reset(&reset).await.map_err(ResetPasswordsError::Broker)
    }
}

#[derive(Debug, Error)]
pub enum ResetPasswordsError<R, A>
where
    R: Error + 'static,
    A: Error + 'static,
{
    #[error(transparent)]
    Broker(PasswordResetError<R>),
    #[error("reset-password action failed")]
    Action(#[source] A),
}
