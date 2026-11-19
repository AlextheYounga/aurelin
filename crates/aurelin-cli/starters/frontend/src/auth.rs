use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use aurelin_fortify::{
    CreatesNewUsers, EmailVerification, EmailVerificationError, EmailVerificationRateLimiter,
    EmailVerificationRequest, LoginRateLimiter, RegistrationInput, ResetPasswords,
    ResetPasswordsError,
};
use aurelin_framework::auth::{
    BcryptPasswordHasher, PasswordHashError, PasswordResetBroker, PasswordResetCredentials,
    PasswordResetError, PasswordResetRepository, PasswordResetRequest, RateLimitOutcome,
    StoredPasswordResetToken,
};
use axum_login::{AuthUser, AuthnBackend, UserId};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, sea_query::OnConflict,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    actions::{
        CreateNewUser, CreateNewUserError, CreateUser, ResetUserPassword, ResetUserPasswordError,
    },
    entities::{password_reset_token, user},
    password_policy::{PasswordPolicy, PasswordPolicyViolation},
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
    password_reset_tokens: PasswordResetBroker<AuthRepository>,
    reset_passwords: ResetPasswords<AuthRepository, ResetUserPassword>,
    create_new_user: CreateNewUser,
    email_verification: EmailVerification,
    email_verification_attempts: EmailVerificationRateLimiter,
    password_policy: PasswordPolicy,
}

impl AuthBackend {
    pub async fn new(
        database: DatabaseConnection,
        application_key: &[u8],
        application_environment: &str,
    ) -> Result<Self, AuthError> {
        let passwords = BcryptPasswordHasher::default();
        let dummy_password_hash = passwords.hash("aurelin-invalid-credentials").await?;
        Ok(Self {
            password_reset_tokens: PasswordResetBroker::new(AuthRepository(database.clone())),
            reset_passwords: ResetPasswords::new(
                AuthRepository(database.clone()),
                ResetUserPassword::new(database.clone(), passwords),
            ),
            create_new_user: CreateNewUser::new(database.clone(), passwords),
            email_verification: EmailVerification::new(application_key)?,
            email_verification_attempts: EmailVerificationRateLimiter::default(),
            password_policy: PasswordPolicy::for_environment(application_environment),
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

    pub async fn login_attempt(&self, key: &str) -> RateLimitOutcome {
        self.attempts.attempt(key).await
    }

    pub fn validate_password(&self, password: &str) -> Result<(), PasswordPolicyViolation> {
        self.password_policy.validate(password)
    }

    pub fn password_rules(&self) -> &'static str {
        self.password_policy.browser_rules()
    }

    pub async fn create_password_reset_token(
        &self,
        email: &str,
    ) -> Result<PasswordResetRequest, AuthError> {
        let email = normalize_email(email);
        Ok(self
            .password_reset_tokens
            .create_token(&email, unix_timestamp()?)
            .await?)
    }

    pub async fn reset_password(
        &self,
        email: &str,
        token: &str,
        password: &str,
    ) -> Result<bool, AuthError> {
        let email = normalize_email(email);
        Ok(self
            .reset_passwords
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

    pub async fn email_verification_attempt(&self, user_id: i64) -> RateLimitOutcome {
        self.email_verification_attempts.attempt(user_id).await
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
    ResetPasswords(#[from] ResetPasswordsError<DbErr, ResetUserPasswordError>),
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

    async fn delete_token(&self, email: &str, expected_digest: &str) -> Result<bool, Self::Error> {
        let result = password_reset_token::Entity::delete_many()
            .filter(password_reset_token::Column::Email.eq(email))
            .filter(password_reset_token::Column::TokenHash.eq(expected_digest))
            .exec(&self.0)
            .await?;
        Ok(result.rows_affected == 1)
    }
}

fn normalize_email(email: &str) -> String {
    email.to_lowercase()
}

fn unix_timestamp() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
