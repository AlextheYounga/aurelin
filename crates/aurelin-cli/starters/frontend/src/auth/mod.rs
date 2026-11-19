use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aurelin_fortify::{
    CreatesNewUsers, EmailVerification, EmailVerificationError, EmailVerificationRateLimiter,
    EmailVerificationRequest, LoginRateLimiter, PasskeyRateLimiter, RegistrationInput,
    ResetPasswords, ResetPasswordsError, TwoFactorAuthentication, TwoFactorError,
    TwoFactorRateLimiter,
    passkeys::{PasskeyConfig, PasskeyError, Passkeys},
};
use aurelin_framework::auth::{
    BcryptPasswordHasher, FixedWindowRateLimiter, PasswordHashError, PasswordResetBroker,
    PasswordResetCredentials, PasswordResetError, PasswordResetRepository, PasswordResetRequest,
    RateLimitOutcome, StoredPasswordResetToken,
};
use aurelin_framework::mail::{MailError, Mailer};
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

mod fortify;
mod notifications;
mod passkeys;
mod settings;
mod two_factor;

pub use settings::ProfileUpdate;

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

pub use aurelin_fortify::axum::Credentials;

#[derive(Clone, Debug)]
pub struct AuthBackend {
    pub(super) database: DatabaseConnection,
    dummy_password_hash: Arc<String>,
    passwords: BcryptPasswordHasher,
    attempts: LoginRateLimiter,
    authentication_timebox_duration: Duration,
    password_reset_tokens: PasswordResetBroker<AuthRepository>,
    reset_passwords: ResetPasswords<AuthRepository, ResetUserPassword>,
    create_new_user: CreateNewUser,
    email_verification: EmailVerification,
    email_verification_attempts: EmailVerificationRateLimiter,
    password_policy: PasswordPolicy,
    password_update_attempts: FixedWindowRateLimiter,
    mailer: Mailer,
    application_url: Arc<String>,
    pub(super) two_factor: TwoFactorAuthentication,
    pub(super) two_factor_attempts: TwoFactorRateLimiter,
    pub(super) passkeys: Passkeys,
    passkey_attempts: PasskeyRateLimiter,
}

impl AuthBackend {
    pub async fn new(
        database: DatabaseConnection,
        application_key: &[u8],
        application_environment: &str,
        application_url: &str,
        mailer: Mailer,
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
            password_update_attempts: FixedWindowRateLimiter::new(6, Duration::from_secs(60)),
            mailer,
            application_url: Arc::new(application_url.trim_end_matches('/').to_owned()),
            two_factor: TwoFactorAuthentication::new(application_key, "Aurelin")?,
            two_factor_attempts: TwoFactorRateLimiter::default(),
            passkeys: Passkeys::new(PasskeyConfig::for_origin(
                "Aurelin",
                application_url,
                application_key,
            )?)?,
            passkey_attempts: PasskeyRateLimiter::default(),
            database,
            dummy_password_hash: Arc::new(dummy_password_hash),
            passwords,
            attempts: LoginRateLimiter::default(),
            authentication_timebox_duration:
                aurelin_fortify::axum::DEFAULT_AUTHENTICATION_TIMEBOX_DURATION,
        })
    }

    pub fn with_authentication_timebox_duration(mut self, duration: Duration) -> Self {
        self.authentication_timebox_duration = duration;
        self
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

    pub async fn login_retry_after(&self, key: &str) -> Option<Duration> {
        self.attempts.retry_after(key).await
    }

    pub async fn record_failed_login(&self, key: &str) -> RateLimitOutcome {
        self.attempts.attempt(key).await
    }

    pub async fn clear_failed_logins(&self, key: &str) {
        self.attempts.clear(key).await;
    }

    pub async fn validate_password(&self, password: &str) -> Result<(), PasswordPolicyViolation> {
        self.password_policy.validate(password).await
    }

    pub fn password_rules(&self) -> &'static str {
        self.password_policy.browser_rules()
    }

    pub fn application_url(&self) -> &str {
        &self.application_url
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

    pub async fn passkey_attempt(&self, key: &str) -> RateLimitOutcome {
        self.passkey_attempts.attempt(key).await
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
        if !self.passwords.verify(&credentials.password, &hash).await? {
            return Ok(None);
        }
        let Some(user) = user else {
            return Ok(None);
        };
        if !self.passwords.needs_rehash(&user.password_hash)? {
            return Ok(Some(user));
        }
        let mut active: user::ActiveModel = user.into();
        active.password_hash = Set(self.passwords.hash(&credentials.password).await?);
        active.updated_at = Set(unix_timestamp()?);
        Ok(Some(active.update(&self.database).await?))
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
    #[error(transparent)]
    TwoFactor(#[from] TwoFactorError),
    #[error(transparent)]
    Passkey(#[from] PasskeyError),
    #[error(transparent)]
    Mail(#[from] MailError),
    #[error("this passkey is already registered")]
    PasskeyAlreadyRegistered,
    #[error("system clock produced an invalid timestamp")]
    InvalidTimestamp,
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

pub(super) fn unix_timestamp() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
