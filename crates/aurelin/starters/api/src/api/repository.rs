use std::time::{SystemTime, UNIX_EPOCH};

use aurelin_sanctum::{Abilities, StoredToken, TokenCreation, TokenMetadata, TokenRepository};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, QueryOrder,
};

use super::personal_access_token::{self as token, Column};

#[derive(Clone)]
pub struct Repository(pub DatabaseConnection);

fn token_id(id: u64) -> Result<i64, DbErr> {
    i64::try_from(id).map_err(|_| DbErr::Custom("invalid token id".into()))
}

fn abilities(value: serde_json::Value) -> Result<Abilities, DbErr> {
    serde_json::from_value(value)
        .map(Abilities::new)
        .map_err(|_| DbErr::Custom("invalid token abilities".into()))
}

impl TokenRepository for Repository {
    type Owner = i64;
    type Error = DbErr;

    async fn create(&self, input: TokenCreation<'_, i64>) -> Result<u64, DbErr> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| DbErr::Custom("invalid system clock".into()))?
            .as_secs();
        let now = i64::try_from(now).map_err(|_| DbErr::Custom("invalid system clock".into()))?;
        let model = token::ActiveModel {
            user_id: Set(*input.owner),
            name: Set(input.name.to_owned()),
            digest: Set(input.digest.to_owned()),
            abilities: Set(serde_json::json!(input.abilities.as_slice())),
            expires_at: Set(input.expires_at),
            last_used_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&self.0)
        .await?;
        u64::try_from(model.id).map_err(|_| DbErr::Custom("invalid token id".into()))
    }

    async fn find(&self, id: u64) -> Result<Option<StoredToken<i64>>, DbErr> {
        let Ok(id) = token_id(id) else {
            return Ok(None);
        };
        token::Entity::find_by_id(id)
            .one(&self.0)
            .await?
            .map(|model| {
                Ok(StoredToken {
                    owner: model.user_id,
                    digest: model.digest,
                    abilities: abilities(model.abilities)?,
                    expires_at: model.expires_at,
                })
            })
            .transpose()
    }

    async fn mark_used(&self, id: u64, digest: &str, now: i64) -> Result<bool, DbErr> {
        let Ok(id) = token_id(id) else {
            return Ok(false);
        };
        let result = token::Entity::update_many()
            .col_expr(Column::LastUsedAt, sea_orm::sea_query::Expr::value(now))
            .col_expr(Column::UpdatedAt, sea_orm::sea_query::Expr::value(now))
            .filter(Column::Id.eq(id))
            .filter(Column::Digest.eq(digest))
            .filter(
                Condition::any()
                    .add(Column::ExpiresAt.is_null())
                    .add(Column::ExpiresAt.gt(now)),
            )
            .exec(&self.0)
            .await?;
        Ok(result.rows_affected == 1)
    }

    async fn revoke(&self, owner: &i64, id: u64) -> Result<bool, DbErr> {
        let Ok(id) = token_id(id) else {
            return Ok(false);
        };
        let result = token::Entity::delete_many()
            .filter(Column::Id.eq(id))
            .filter(Column::UserId.eq(*owner))
            .exec(&self.0)
            .await?;
        Ok(result.rows_affected == 1)
    }

    async fn list(&self, owner: &i64) -> Result<Vec<TokenMetadata>, DbErr> {
        token::Entity::find()
            .filter(Column::UserId.eq(*owner))
            .order_by_asc(Column::Id)
            .all(&self.0)
            .await?
            .into_iter()
            .map(|model| {
                Ok(TokenMetadata {
                    id: u64::try_from(model.id)
                        .map_err(|_| DbErr::Custom("invalid token id".into()))?,
                    name: model.name,
                    abilities: abilities(model.abilities)?,
                    expires_at: model.expires_at,
                    last_used_at: model.last_used_at,
                })
            })
            .collect()
    }
}
