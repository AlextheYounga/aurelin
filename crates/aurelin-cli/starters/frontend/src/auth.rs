use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum_login::{AuthUser, AuthnBackend, UserId};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, SqlErr, TransactionTrait, sea_query::OnConflict,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinError};

use crate::entities::{email_verification_token, password_reset_token, user};

const TOKEN_LIFETIME_SECONDS: i64 = 60 * 60;

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
    attempts: LoginAttempts,
}

impl AuthBackend {
    pub async fn new(database: DatabaseConnection) -> Result<Self, AuthError> {
        let dummy_password_hash = hash_password("aurelin-invalid-credentials").await?;
        Ok(Self {
            database,
            dummy_password_hash: Arc::new(dummy_password_hash),
            attempts: LoginAttempts::new()?,
        })
    }

    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> Result<CreateUser, AuthError> {
        let now = unix_timestamp()?;
        let active_user = user::ActiveModel {
            name: Set(name.to_owned()),
            email: Set(normalize_email(email)),
            password_hash: Set(hash_password(password).await?),
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

    pub async fn too_many_login_attempts(&self, key: &str) -> bool {
        self.attempts.check(key).await
    }

    pub async fn clear_login_attempts(&self, key: &str) {
        self.attempts.clear(key).await;
    }

    pub async fn create_password_reset_token(
        &self,
        email: &str,
    ) -> Result<Option<String>, AuthError> {
        let email = normalize_email(email);
        if user::Entity::find()
            .filter(user::Column::Email.eq(&email))
            .one(&self.database)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let token = axum_tower_sessions_csrf::generate_token();
        password_reset_token::Entity::insert(password_reset_token::ActiveModel {
            email: Set(email),
            token_hash: Set(token_digest(&token)),
            created_at: Set(unix_timestamp()?),
        })
        .on_conflict(
            OnConflict::column(password_reset_token::Column::Email)
                .update_columns([
                    password_reset_token::Column::TokenHash,
                    password_reset_token::Column::CreatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.database)
        .await?;
        Ok(Some(token))
    }

    pub async fn reset_password(
        &self,
        email: &str,
        token: &str,
        password: &str,
    ) -> Result<bool, AuthError> {
        let email = normalize_email(email);
        let transaction = self.database.begin().await?;
        let Some(reset_token) = password_reset_token::Entity::find_by_id(&email)
            .one(&transaction)
            .await?
        else {
            return Ok(false);
        };
        if reset_token.token_hash != token_digest(token)
            || reset_token.created_at < unix_timestamp()? - TOKEN_LIFETIME_SECONDS
        {
            return Ok(false);
        }
        let Some(user) = user::Entity::find()
            .filter(user::Column::Email.eq(&email))
            .one(&transaction)
            .await?
        else {
            return Ok(false);
        };

        let mut user: user::ActiveModel = user.into();
        user.password_hash = Set(hash_password(password).await?);
        user.updated_at = Set(unix_timestamp()?);
        user.update(&transaction).await?;
        password_reset_token::Entity::delete_by_id(&email)
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn create_email_verification_token(&self, user_id: i64) -> Result<String, AuthError> {
        let token = axum_tower_sessions_csrf::generate_token();
        email_verification_token::Entity::insert(email_verification_token::ActiveModel {
            user_id: Set(user_id),
            token_hash: Set(token_digest(&token)),
            created_at: Set(unix_timestamp()?),
        })
        .on_conflict(
            OnConflict::column(email_verification_token::Column::UserId)
                .update_columns([
                    email_verification_token::Column::TokenHash,
                    email_verification_token::Column::CreatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.database)
        .await?;
        Ok(token)
    }

    pub async fn verify_email(&self, user_id: i64, token: &str) -> Result<bool, AuthError> {
        let transaction = self.database.begin().await?;
        let Some(verification_token) = email_verification_token::Entity::find_by_id(user_id)
            .one(&transaction)
            .await?
        else {
            return Ok(false);
        };
        if verification_token.token_hash != token_digest(token)
            || verification_token.created_at < unix_timestamp()? - TOKEN_LIFETIME_SECONDS
        {
            return Ok(false);
        }
        let Some(user) = user::Entity::find_by_id(user_id).one(&transaction).await? else {
            return Ok(false);
        };

        let mut user: user::ActiveModel = user.into();
        user.email_verified_at = Set(Some(unix_timestamp()?));
        user.updated_at = Set(unix_timestamp()?);
        user.update(&transaction).await?;
        email_verification_token::Entity::delete_by_id(user_id)
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
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
        let password_matches = verify_password(credentials.password, hash).await?;
        Ok(user.filter(|_| password_matches))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(user::Entity::find_by_id(*user_id)
            .one(&self.database)
            .await?)
    }
}

pub enum CreateUser {
    Created(User),
    EmailAlreadyExists,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("database operation failed")]
    Database(#[from] DbErr),
    #[error("password operation failed")]
    Password,
    #[error("password worker failed")]
    PasswordWorker(#[from] JoinError),
    #[error("system clock is before the Unix epoch")]
    Clock(#[from] std::time::SystemTimeError),
    #[error("login rate limit could not be configured")]
    RateLimitConfiguration,
}

async fn hash_password(password: &str) -> Result<String, AuthError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|_| AuthError::Password)
    })
    .await?
}

async fn verify_password(password: String, password_hash: String) -> Result<bool, AuthError> {
    tokio::task::spawn_blocking(move || {
        let hash = PasswordHash::new(&password_hash).map_err(|_| AuthError::Password)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok())
    })
    .await?
}

fn normalize_email(email: &str) -> String {
    email.to_lowercase()
}

fn token_digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn unix_timestamp() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}

#[derive(Clone, Debug)]
struct LoginAttempts {
    quota: Quota,
    entries: Arc<Mutex<HashMap<String, Arc<DefaultDirectRateLimiter>>>>,
}

impl LoginAttempts {
    fn new() -> Result<Self, AuthError> {
        let attempts = NonZeroU32::new(5).ok_or(AuthError::RateLimitConfiguration)?;
        Ok(Self {
            quota: Quota::per_minute(attempts),
            entries: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn check(&self, key: &str) -> bool {
        let limiter = self
            .entries
            .lock()
            .await
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(RateLimiter::direct(self.quota)))
            .clone();
        limiter.check().is_err()
    }

    async fn clear(&self, key: &str) {
        self.entries.lock().await.remove(key);
    }
}
