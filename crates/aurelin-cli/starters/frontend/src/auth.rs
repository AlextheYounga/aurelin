use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use aurelin_fortify::{
    CreatesNewUsers, EmailVerification, EmailVerificationError, EmailVerificationRequest,
    LoginRateLimiter, RegistrationInput,
};
use aurelin_framework::auth::{
    BcryptPasswordHasher, ConsumedPasswordReset, PasswordHashError, PasswordResetBroker,
    PasswordResetCredentials, PasswordResetError, PasswordResetRepository, PasswordResetRequest,
    StoredPasswordResetToken,
};
use axum_login::{AuthUser, AuthnBackend, UserId};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, TransactionTrait, sea_query::OnConflict,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    actions::{CreateNewUser, CreateNewUserError, CreateUser},
    entities::{password_reset_token, user},
};

pub type AuthSession = axum_login::AuthSession<AuthBackend>;
pub type User = user::Model;

#[derive(Clone, Debug, Serialize)]
pub struct PublicUser {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub email_verified_at: Option<i64>,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            name: user.name.clone(),
            email: user.email.clone(),
            email_verified_at: user.email_verified_at,
        }
    }
}

impl AuthUser for User {
    type Id = i64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes()
    }
}

#[derive(Clone)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug)]
pub struct AuthBackend {
    database: DatabaseConnection,
    dummy_password_hash: Arc<String>,
    passwords: BcryptPasswordHasher,
    attempts: LoginRateLimiter,
    password_resets: PasswordResetBroker<AuthRepository>,
    create_new_user: CreateNewUser,
    email_verification: EmailVerification,
}

impl AuthBackend {
    pub async fn new(
        database: DatabaseConnection,
        application_key: &[u8],
    ) -> Result<Self, AuthError> {
        let passwords = BcryptPasswordHasher::default();
        let dummy_password_hash = passwords.hash("aurelin-invalid-credentials").await?;
        Ok(Self {
            password_resets: PasswordResetBroker::new(AuthRepository(database.clone())),
            create_new_user: CreateNewUser::new(database.clone(), passwords),
            email_verification: EmailVerification::new(application_key)?,
            database,
            dummy_password_hash: Arc::new(dummy_password_hash),
            passwords,
            attempts: LoginRateLimiter::default(),
        })
    }

    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> Result<CreateUser, AuthError> {
        Ok(self
            .create_new_user
            .create(RegistrationInput {
                name: name.to_owned(),
                email: email.to_owned(),
                password: password.to_owned(),
            })
            .await?)
    }

    pub async fn too_many_login_attempts(&self, key: &str) -> bool {
        if self.attempts.retry_after(key).await.is_some() {
            return true;
        }
        self.attempts.hit(key).await;
        false
    }

    pub async fn clear_login_attempts(&self, key: &str) {
        self.attempts.clear(key).await;
    }

    pub async fn create_password_reset_token(
        &self,
        email: &str,
    ) -> Result<Option<String>, AuthError> {
        let email = normalize_email(email);
        Ok(
            match self
                .password_resets
                .create_token(&email, unix_timestamp()?)
                .await?
            {
                PasswordResetRequest::Sent { token } => Some(token),
                PasswordResetRequest::UnknownUser | PasswordResetRequest::Throttled => None,
            },
        )
    }

    pub async fn reset_password(
        &self,
        email: &str,
        token: &str,
        password: &str,
    ) -> Result<bool, AuthError> {
        let email = normalize_email(email);
        Ok(self
            .password_resets
            .reset(
                PasswordResetCredentials {
                    email: &email,
                    token,
                    password,
                },
                unix_timestamp()?,
            )
            .await?)
    }

    pub fn email_verification_url(&self, user: &User) -> Result<String, AuthError> {
        Ok(self
            .email_verification
            .url(user.id, &user.email, unix_timestamp()?)?)
    }

    pub async fn verify_email(
        &self,
        user: &User,
        email_hash: &str,
        expires: i64,
        signature: &str,
    ) -> Result<bool, AuthError> {
        if !self.email_verification.verify(
            EmailVerificationRequest {
                user_id: user.id,
                email: &user.email,
                email_hash,
                expires,
                signature,
            },
            unix_timestamp()?,
        )? {
            return Ok(false);
        }
        if user.email_verified_at.is_some() {
            return Ok(true);
        }
        let mut user: user::ActiveModel = user.clone().into();
        user.email_verified_at = Set(Some(unix_timestamp()?));
        user.updated_at = Set(unix_timestamp()?);
        user.update(&self.database).await?;
        Ok(true)
    }
}

impl AuthnBackend for AuthBackend {
    type User = User;
    type Credentials = Credentials;
    type Error = AuthError;

    async fn authenticate(
        &self,
        credentials: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = user::Entity::find()
            .filter(user::Column::Email.eq(normalize_email(&credentials.email)))
            .one(&self.database)
            .await?;
        let hash = user
            .as_ref()
            .map_or_else(
                || self.dummy_password_hash.as_str(),
                |user| user.password_hash.as_str(),
            )
            .to_owned();
        let password_matches = self.passwords.verify(&credentials.password, &hash).await?;
        Ok(user.filter(|_| password_matches))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(user::Entity::find_by_id(*user_id)
            .one(&self.database)
            .await?)
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("database operation failed")]
    Database(#[from] DbErr),
    #[error(transparent)]
    Password(#[from] PasswordHashError),
    #[error(transparent)]
    PasswordReset(#[from] PasswordResetError<DbErr>),
    #[error(transparent)]
    CreateNewUser(#[from] CreateNewUserError),
    #[error(transparent)]
    EmailVerification(#[from] EmailVerificationError),
    #[error("system clock is before the Unix epoch")]
    Clock(#[from] std::time::SystemTimeError),
}

#[derive(Clone, Debug)]
struct AuthRepository(DatabaseConnection);

impl PasswordResetRepository for AuthRepository {
    type Error = DbErr;

    async fn user_exists(&self, email: &str) -> Result<bool, Self::Error> {
        Ok(user::Entity::find()
            .filter(user::Column::Email.eq(email))
            .one(&self.0)
            .await?
            .is_some())
    }

    async fn token(&self, email: &str) -> Result<Option<StoredPasswordResetToken>, Self::Error> {
        Ok(password_reset_token::Entity::find_by_id(email)
            .one(&self.0)
            .await?
            .map(|token| StoredPasswordResetToken {
                digest: token.token_hash,
                created_at: token.created_at,
            }))
    }

    async fn replace_token(
        &self,
        email: &str,
        token: StoredPasswordResetToken,
    ) -> Result<(), Self::Error> {
        password_reset_token::Entity::insert(password_reset_token::ActiveModel {
            email: Set(email.to_owned()),
            token_hash: Set(token.digest),
            created_at: Set(token.created_at),
        })
        .on_conflict(
            OnConflict::column(password_reset_token::Column::Email)
                .update_columns([
                    password_reset_token::Column::TokenHash,
                    password_reset_token::Column::CreatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.0)
        .await?;
        Ok(())
    }

    async fn consume_token(&self, reset: ConsumedPasswordReset<'_>) -> Result<bool, Self::Error> {
        let transaction = self.0.begin().await?;
        let Some(token) = password_reset_token::Entity::find_by_id(reset.email)
            .one(&transaction)
            .await?
        else {
            return Ok(false);
        };
        if token.token_hash != reset.expected_digest {
            return Ok(false);
        }
        let Some(user) = user::Entity::find()
            .filter(user::Column::Email.eq(reset.email))
            .one(&transaction)
            .await?
        else {
            return Ok(false);
        };
        let mut user: user::ActiveModel = user.into();
        user.password_hash = Set(reset.password_hash.to_owned());
        user.updated_at = Set(reset.updated_at);
        user.update(&transaction).await?;
        password_reset_token::Entity::delete_by_id(reset.email)
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }
}

fn normalize_email(email: &str) -> String {
    email.to_lowercase()
}

fn unix_timestamp() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
