use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum_login::{AuthUser, AuthnBackend, UserId};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinError};

const MAX_LOGIN_ATTEMPTS: usize = 5;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);

pub type AuthSession = axum_login::AuthSession<AuthBackend>;

#[derive(Clone, Debug, FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    password_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicUser {
    pub id: i64,
    pub name: String,
    pub email: String,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            name: user.name.clone(),
            email: user.email.clone(),
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
    pool: SqlitePool,
    dummy_password_hash: Arc<String>,
    attempts: LoginAttempts,
}

impl AuthBackend {
    pub async fn new(pool: SqlitePool) -> Result<Self, AuthError> {
        let dummy_password_hash = hash_password("kiln-invalid-credentials").await?;
        Ok(Self {
            pool,
            dummy_password_hash: Arc::new(dummy_password_hash),
            attempts: LoginAttempts::default(),
        })
    }

    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> Result<CreateUser, AuthError> {
        let password_hash = hash_password(password).await?;
        let result = sqlx::query_as::<_, User>(
            "INSERT INTO users (name, email, password_hash) VALUES (?, ?, ?) \
             RETURNING id, name, email, password_hash",
        )
        .bind(name)
        .bind(email)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(user) => Ok(CreateUser::Created(user)),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                Ok(CreateUser::EmailAlreadyExists)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn too_many_login_attempts(&self, key: &str) -> bool {
        self.attempts.too_many(key).await
    }

    pub async fn record_failed_login(&self, key: String) {
        self.attempts.record(key).await;
    }

    pub async fn clear_login_attempts(&self, key: &str) {
        self.attempts.clear(key).await;
    }

    pub async fn create_password_reset_token(
        &self,
        email: &str,
    ) -> Result<Option<String>, AuthError> {
        let user_exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE email = ?")
                .bind(email)
                .fetch_one(&self.pool)
                .await?
                > 0;
        if !user_exists {
            return Ok(None);
        }

        let token = axum_tower_sessions_csrf::generate_token();
        sqlx::query(
            "INSERT INTO password_reset_tokens (email, token_hash, created_at) \
             VALUES (?, ?, CURRENT_TIMESTAMP) \
             ON CONFLICT(email) DO UPDATE SET token_hash = excluded.token_hash, created_at = CURRENT_TIMESTAMP",
        )
        .bind(email)
        .bind(token_digest(&token))
        .execute(&self.pool)
        .await?;
        Ok(Some(token))
    }

    pub async fn reset_password(
        &self,
        email: &str,
        token: &str,
        password: &str,
    ) -> Result<bool, AuthError> {
        let mut transaction = self.pool.begin().await?;
        let valid = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM password_reset_tokens \
             WHERE email = ? AND token_hash = ? AND created_at >= datetime('now', '-60 minutes')",
        )
        .bind(email)
        .bind(token_digest(token))
        .fetch_one(&mut *transaction)
        .await?
            > 0;
        if !valid {
            return Ok(false);
        }

        let password_hash = hash_password(password).await?;
        sqlx::query(
            "UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE email = ?",
        )
        .bind(password_hash)
        .bind(email)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM password_reset_tokens WHERE email = ?")
            .bind(email)
            .execute(&mut *transaction)
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
        let user = sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash FROM users WHERE email = ? LIMIT 1",
        )
        .bind(credentials.email)
        .fetch_optional(&self.pool)
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
        Ok(sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash FROM users WHERE id = ? LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
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
    Database(#[from] sqlx::Error),
    #[error("password operation failed")]
    Password,
    #[error("password worker failed")]
    PasswordWorker(#[from] JoinError),
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

#[derive(Clone, Debug, Default)]
struct LoginAttempts {
    entries: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl LoginAttempts {
    async fn too_many(&self, key: &str) -> bool {
        let mut entries = self.entries.lock().await;
        let Some(attempts) = entries.get_mut(key) else {
            return false;
        };
        remove_expired(attempts);
        attempts.len() >= MAX_LOGIN_ATTEMPTS
    }

    async fn record(&self, key: String) {
        let mut entries = self.entries.lock().await;
        let attempts = entries.entry(key).or_default();
        remove_expired(attempts);
        attempts.push_back(Instant::now());
    }

    async fn clear(&self, key: &str) {
        self.entries.lock().await.remove(key);
    }
}

fn remove_expired(attempts: &mut VecDeque<Instant>) {
    while attempts
        .front()
        .is_some_and(|attempt| attempt.elapsed() >= LOGIN_ATTEMPT_WINDOW)
    {
        attempts.pop_front();
    }
}

fn token_digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
