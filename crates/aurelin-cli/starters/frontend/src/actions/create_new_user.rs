use std::time::{SystemTime, UNIX_EPOCH};

use aurelin_fortify::{CreatesNewUsers, RegistrationInput};
use aurelin_framework::auth::{BcryptPasswordHasher, PasswordHashError};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, DbErr, SqlErr};
use thiserror::Error;

use crate::entities::user;

#[derive(Clone, Debug)]
pub struct CreateNewUser {
    database: DatabaseConnection,
    passwords: BcryptPasswordHasher,
}

impl CreateNewUser {
    pub fn new(database: DatabaseConnection, passwords: BcryptPasswordHasher) -> Self {
        Self {
            database,
            passwords,
        }
    }
}

impl CreatesNewUsers for CreateNewUser {
    type User = CreateUser;
    type Error = CreateNewUserError;

    async fn create(&self, input: RegistrationInput) -> Result<Self::User, Self::Error> {
        let now = unix_timestamp()?;
        let active_user = user::ActiveModel {
            name: Set(input.name),
            email: Set(input.email.to_lowercase()),
            password_hash: Set(self.passwords.hash(&input.password).await?),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        match active_user.insert(&self.database).await {
            Ok(user) => Ok(CreateUser::Created(user)),
            Err(error) if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) => {
                Ok(CreateUser::EmailAlreadyExists)
            }
            Err(error) => Err(error.into()),
        }
    }
}

pub enum CreateUser {
    Created(user::Model),
    EmailAlreadyExists,
}

#[derive(Debug, Error)]
pub enum CreateNewUserError {
    #[error("database operation failed")]
    Database(#[from] DbErr),
    #[error(transparent)]
    Password(#[from] PasswordHashError),
    #[error("system clock is before the Unix epoch")]
    Clock(#[from] std::time::SystemTimeError),
}

fn unix_timestamp() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
