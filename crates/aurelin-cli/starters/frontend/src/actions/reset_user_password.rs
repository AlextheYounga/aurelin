use std::time::{SystemTime, UNIX_EPOCH};

use aurelin_fortify::{ResetPasswordInput, ResetsUserPasswords};
use aurelin_framework::auth::{BcryptPasswordHasher, PasswordHashError};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter,
};
use thiserror::Error;

use crate::entities::user;

#[derive(Clone, Debug)]
pub struct ResetUserPassword {
    database: DatabaseConnection,
    passwords: BcryptPasswordHasher,
}

impl ResetUserPassword {
    pub fn new(database: DatabaseConnection, passwords: BcryptPasswordHasher) -> Self {
        Self {
            database,
            passwords,
        }
    }
}

impl ResetsUserPasswords for ResetUserPassword {
    type Error = ResetUserPasswordError;

    async fn reset(&self, input: ResetPasswordInput) -> Result<(), Self::Error> {
        let Some(user) = user::Entity::find()
            .filter(user::Column::Email.eq(input.email))
            .one(&self.database)
            .await?
        else {
            return Err(ResetUserPasswordError::UserNotFound);
        };
        let mut user: user::ActiveModel = user.into();
        user.password_hash = Set(self.passwords.hash(&input.password).await?);
        user.updated_at = Set(unix_timestamp()?);
        user.update(&self.database).await?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ResetUserPasswordError {
    #[error("database operation failed")]
    Database(#[from] DbErr),
    #[error(transparent)]
    Password(#[from] PasswordHashError),
    #[error("password reset user no longer exists")]
    UserNotFound,
    #[error("system clock is before the Unix epoch")]
    Clock(#[from] std::time::SystemTimeError),
}

fn unix_timestamp() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
