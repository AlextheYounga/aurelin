use aurelin_fortify::{TwoFactorChallenge, TwoFactorChallengeOutcome, TwoFactorState};
use aurelin_framework::auth::RateLimitOutcome;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};

use super::{AuthBackend, AuthError, User, unix_timestamp};
use crate::entities::user;

impl AuthBackend {
    pub async fn enable_two_factor(&self, user: &User) -> Result<User, AuthError> {
        if user.two_factor_secret.is_some() {
            return Ok(user.clone());
        }
        let setup = self.two_factor.enable()?;
        let mut active: user::ActiveModel = user.clone().into();
        active.two_factor_secret = Set(Some(setup.encrypted_secret));
        active.two_factor_recovery_codes = Set(Some(setup.encrypted_recovery_codes));
        active.two_factor_confirmed_at = Set(None);
        active.updated_at = Set(unix_timestamp()?);
        Ok(active.update(&self.database).await?)
    }

    pub async fn confirm_two_factor(&self, user: &User, code: &str) -> Result<bool, AuthError> {
        let Some(secret) = user.two_factor_secret.as_deref() else {
            return Ok(false);
        };
        let timestamp =
            u64::try_from(unix_timestamp()?).map_err(|_| AuthError::InvalidTimestamp)?;
        if !self.two_factor.confirm(secret, code, timestamp)? {
            return Ok(false);
        }
        let mut active: user::ActiveModel = user.clone().into();
        active.two_factor_confirmed_at = Set(Some(unix_timestamp()?));
        active.updated_at = Set(unix_timestamp()?);
        active.update(&self.database).await?;
        Ok(true)
    }

    pub async fn disable_two_factor(&self, user: &User) -> Result<(), AuthError> {
        let mut active: user::ActiveModel = user.clone().into();
        active.two_factor_secret = Set(None);
        active.two_factor_recovery_codes = Set(None);
        active.two_factor_confirmed_at = Set(None);
        active.updated_at = Set(unix_timestamp()?);
        active.update(&self.database).await?;
        Ok(())
    }

    pub fn two_factor_secret_key(&self, user: &User) -> Result<Option<String>, AuthError> {
        user.two_factor_secret
            .as_deref()
            .map(|secret| self.two_factor.secret_key(secret).map_err(AuthError::from))
            .transpose()
    }

    pub fn two_factor_qr_code(&self, user: &User) -> Result<Option<String>, AuthError> {
        user.two_factor_secret
            .as_deref()
            .map(|secret| {
                self.two_factor
                    .qr_code(secret, &user.email)
                    .map_err(AuthError::from)
            })
            .transpose()
    }

    pub fn two_factor_recovery_codes(&self, user: &User) -> Result<Vec<String>, AuthError> {
        user.two_factor_recovery_codes
            .as_deref()
            .map(|codes| {
                self.two_factor
                    .recovery_codes(codes)
                    .map_err(AuthError::from)
            })
            .transpose()
            .map(Option::unwrap_or_default)
    }

    pub async fn regenerate_two_factor_recovery_codes(&self, user: &User) -> Result<(), AuthError> {
        if user.two_factor_secret.is_none() {
            return Ok(());
        }
        let mut active: user::ActiveModel = user.clone().into();
        active.two_factor_recovery_codes = Set(Some(self.two_factor.regenerate_recovery_codes()?));
        active.updated_at = Set(unix_timestamp()?);
        active.update(&self.database).await?;
        Ok(())
    }

    pub async fn challenged_user(&self, user_id: i64) -> Result<Option<User>, AuthError> {
        Ok(user::Entity::find_by_id(user_id)
            .one(&self.database)
            .await?)
    }

    pub async fn challenge_two_factor(
        &self,
        user: &User,
        code: Option<&str>,
        recovery_code: Option<&str>,
    ) -> Result<bool, AuthError> {
        let (Some(secret), Some(recovery_codes)) = (
            user.two_factor_secret.as_deref(),
            user.two_factor_recovery_codes.as_deref(),
        ) else {
            return Ok(false);
        };
        let timestamp =
            u64::try_from(unix_timestamp()?).map_err(|_| AuthError::InvalidTimestamp)?;
        match self.two_factor.challenge(
            TwoFactorState {
                encrypted_secret: secret,
                encrypted_recovery_codes: recovery_codes,
            },
            TwoFactorChallenge {
                code,
                recovery_code,
            },
            timestamp,
        )? {
            TwoFactorChallengeOutcome::CodeAccepted => Ok(true),
            TwoFactorChallengeOutcome::RecoveryCodeAccepted {
                encrypted_recovery_codes,
            } => {
                let mut active: user::ActiveModel = user.clone().into();
                active.two_factor_recovery_codes = Set(Some(encrypted_recovery_codes));
                active.updated_at = Set(unix_timestamp()?);
                active.update(&self.database).await?;
                Ok(true)
            }
            TwoFactorChallengeOutcome::Invalid => Ok(false),
        }
    }

    pub async fn two_factor_attempt(&self, user_id: i64) -> RateLimitOutcome {
        self.two_factor_attempts.attempt(user_id).await
    }
}
