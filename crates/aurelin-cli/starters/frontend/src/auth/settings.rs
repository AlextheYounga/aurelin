use aurelin_framework::auth::RateLimitOutcome;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::{AuthBackend, AuthError, User, normalize_email, unix_timestamp};
use crate::entities::user;

impl AuthBackend {
    pub async fn update_profile(
        &self,
        user: &User,
        name: String,
        email: String,
    ) -> Result<ProfileUpdate, AuthError> {
        let email = normalize_email(&email);
        let email_owner = user::Entity::find()
            .filter(user::Column::Email.eq(&email))
            .one(&self.database)
            .await?;
        if email_owner.is_some_and(|owner| owner.id != user.id) {
            return Ok(ProfileUpdate::EmailAlreadyExists);
        }

        let email_changed = user.email != email;
        let mut active: user::ActiveModel = user.clone().into();
        active.name = Set(name);
        active.email = Set(email);
        if email_changed {
            active.email_verified_at = Set(None);
        }
        active.updated_at = Set(unix_timestamp()?);
        Ok(ProfileUpdate::Updated(active.update(&self.database).await?))
    }

    pub async fn update_password(
        &self,
        user: &User,
        current_password: &str,
        password: &str,
    ) -> Result<Option<User>, AuthError> {
        if !self
            .passwords
            .verify(current_password, &user.password_hash)
            .await?
        {
            return Ok(None);
        }
        let mut active: user::ActiveModel = user.clone().into();
        active.password_hash = Set(self.passwords.hash(password).await?);
        active.updated_at = Set(unix_timestamp()?);
        Ok(Some(active.update(&self.database).await?))
    }

    pub async fn delete_account(&self, user: &User, password: &str) -> Result<bool, AuthError> {
        if !self.passwords.verify(password, &user.password_hash).await? {
            return Ok(false);
        }
        user::Entity::delete_by_id(user.id)
            .exec(&self.database)
            .await?;
        Ok(true)
    }

    pub async fn password_update_attempt(&self, user_id: i64) -> RateLimitOutcome {
        self.password_update_attempts
            .attempt(&user_id.to_string())
            .await
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileUpdate {
    Updated(User),
    EmailAlreadyExists,
}
