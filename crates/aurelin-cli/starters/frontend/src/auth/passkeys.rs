use aurelin_fortify::passkeys::{
    PasskeyAuthenticationRequest, PasskeyCeremony, PasskeyUser, StoredPasskey,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::Value;

use super::{AuthBackend, AuthError, User, unix_timestamp};
use crate::entities::{passkey, user};

impl AuthBackend {
    pub async fn passkeys_for_user(&self, user_id: i64) -> Result<Vec<passkey::Model>, AuthError> {
        Ok(passkey::Entity::find()
            .filter(passkey::Column::UserId.eq(user_id))
            .order_by_desc(passkey::Column::CreatedAt)
            .all(&self.database)
            .await?)
    }

    pub async fn start_passkey_registration(
        &self,
        user: &User,
    ) -> Result<PasskeyCeremony, AuthError> {
        let existing = self.passkeys_for_user(user.id).await?;
        let existing = existing
            .into_iter()
            .map(|passkey| StoredPasskey {
                credential: passkey.credential,
            })
            .collect::<Vec<_>>();
        Ok(self
            .passkeys
            .start_registration(&passkey_user(user), &existing)?)
    }

    pub async fn finish_passkey_registration(
        &self,
        ceremony_id: &str,
        user: &User,
        name: String,
        credential: Value,
    ) -> Result<passkey::Model, AuthError> {
        let verified =
            self.passkeys
                .finish_registration(ceremony_id, &passkey_user(user), credential)?;
        if passkey::Entity::find()
            .filter(passkey::Column::CredentialId.eq(&verified.credential_id))
            .one(&self.database)
            .await?
            .is_some()
        {
            return Err(AuthError::PasskeyAlreadyRegistered);
        }
        let now = unix_timestamp()?;
        Ok(passkey::ActiveModel {
            user_id: Set(user.id),
            name: Set(name),
            credential_id: Set(verified.credential_id),
            credential: Set(verified.credential),
            last_used_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&self.database)
        .await?)
    }

    pub fn start_passkey_login(&self) -> Result<PasskeyCeremony, AuthError> {
        Ok(self.passkeys.start_login()?)
    }

    pub async fn start_passkey_confirmation(
        &self,
        user: &User,
    ) -> Result<PasskeyCeremony, AuthError> {
        let stored = self.passkeys_for_user(user.id).await?;
        let stored = stored
            .into_iter()
            .map(|passkey| StoredPasskey {
                credential: passkey.credential,
            })
            .collect::<Vec<_>>();
        Ok(self
            .passkeys
            .start_confirmation(&passkey_user(user), &stored)?)
    }

    pub async fn finish_passkey_login(
        &self,
        ceremony_id: &str,
        credential: Value,
    ) -> Result<Option<User>, AuthError> {
        let credential_id = self.passkeys.credential_id(credential.clone())?;
        let Some(stored) = passkey::Entity::find()
            .filter(passkey::Column::CredentialId.eq(credential_id))
            .one(&self.database)
            .await?
        else {
            return Ok(None);
        };
        let Some(user) = user::Entity::find_by_id(stored.user_id)
            .one(&self.database)
            .await?
        else {
            return Ok(None);
        };
        let verified = self
            .passkeys
            .finish_authentication(PasskeyAuthenticationRequest {
                ceremony_id,
                user: &passkey_user(&user),
                credential,
                stored_passkey: &StoredPasskey {
                    credential: stored.credential.clone(),
                },
            })?;
        let now = unix_timestamp()?;
        let mut active: passkey::ActiveModel = stored.into();
        active.credential = Set(verified.credential);
        active.last_used_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.database).await?;
        Ok(Some(user))
    }

    pub async fn finish_passkey_confirmation(
        &self,
        ceremony_id: &str,
        user: &User,
        credential: Value,
    ) -> Result<bool, AuthError> {
        let credential_id = self.passkeys.credential_id(credential.clone())?;
        let Some(stored) = passkey::Entity::find()
            .filter(passkey::Column::CredentialId.eq(credential_id))
            .filter(passkey::Column::UserId.eq(user.id))
            .one(&self.database)
            .await?
        else {
            return Ok(false);
        };
        let verified = self
            .passkeys
            .finish_authentication(PasskeyAuthenticationRequest {
                ceremony_id,
                user: &passkey_user(user),
                credential,
                stored_passkey: &StoredPasskey {
                    credential: stored.credential.clone(),
                },
            })?;
        let now = unix_timestamp()?;
        let mut active: passkey::ActiveModel = stored.into();
        active.credential = Set(verified.credential);
        active.last_used_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.database).await?;
        Ok(true)
    }

    pub async fn delete_passkey(&self, user_id: i64, passkey_id: i64) -> Result<bool, AuthError> {
        let result = passkey::Entity::delete_many()
            .filter(passkey::Column::Id.eq(passkey_id))
            .filter(passkey::Column::UserId.eq(user_id))
            .exec(&self.database)
            .await?;
        Ok(result.rows_affected == 1)
    }
}

fn passkey_user(user: &User) -> PasskeyUser {
    PasskeyUser {
        id: user.id.to_string(),
        username: user.email.clone(),
        display_name: user.name.clone(),
    }
}
