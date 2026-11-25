use aurelin_sanctum::{Abilities, StoredToken, TokenCreation, TokenMetadata, TokenRepository};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, Schema,
};

mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "personal_access_tokens")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub owner_id: i64,
        pub name: String,
        pub digest: String,
        pub abilities: Json,
        pub expires_at: Option<i64>,
        pub last_used_at: Option<i64>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Clone)]
pub struct Repository(pub DatabaseConnection);

impl Repository {
    pub async fn new() -> Result<Self, DbErr> {
        let db = Database::connect("sqlite::memory:").await?;
        let schema = Schema::new(db.get_database_backend());
        db.execute(db.get_database_backend().build(&schema.create_table_from_entity(entity::Entity))).await?;
        Ok(Self(db))
    }

    pub async fn stored_digest(&self, id: u64) -> Result<Option<String>, DbErr> {
        Ok(entity::Entity::find_by_id(db_id(id)?).one(&self.0).await?.map(|model| model.digest))
    }
}

fn db_id(id: u64) -> Result<i64, DbErr> {
    i64::try_from(id).map_err(|_| DbErr::Custom("invalid token id".into()))
}

fn metadata(model: entity::Model) -> Result<TokenMetadata, DbErr> {
    Ok(TokenMetadata {
        id: u64::try_from(model.id).map_err(|_| DbErr::Custom("invalid token id".into()))?,
        name: model.name,
        abilities: Abilities::new(
            serde_json::from_value(model.abilities).map_err(|_| DbErr::Custom("invalid abilities".into()))?,
        ),
        expires_at: model.expires_at,
        last_used_at: model.last_used_at,
    })
}

impl TokenRepository for Repository {
    type Owner = i64;
    type Error = DbErr;

    async fn create(&self, token: TokenCreation<'_, i64>) -> Result<u64, DbErr> {
        let model = entity::ActiveModel {
            owner_id: Set(*token.owner),
            name: Set(token.name.to_owned()),
            digest: Set(token.digest.to_owned()),
            abilities: Set(serde_json::json!(token.abilities.as_slice())),
            expires_at: Set(token.expires_at),
            last_used_at: Set(None),
            ..Default::default()
        }
        .insert(&self.0)
        .await?;
        u64::try_from(model.id).map_err(|_| DbErr::Custom("invalid token id".into()))
    }

    async fn find(&self, id: u64) -> Result<Option<StoredToken<i64>>, DbErr> {
        let Some(model) = entity::Entity::find_by_id(db_id(id)?).one(&self.0).await? else { return Ok(None) };
        let abilities =
            serde_json::from_value(model.abilities).map_err(|_| DbErr::Custom("invalid abilities".into()))?;
        Ok(Some(StoredToken {
            owner: model.owner_id,
            digest: model.digest,
            abilities: Abilities::new(abilities),
            expires_at: model.expires_at,
        }))
    }

    async fn mark_used(&self, id: u64, digest: &str, now: i64) -> Result<bool, DbErr> {
        use sea_orm::sea_query::{Condition, Expr};
        let result = entity::Entity::update_many()
            .col_expr(entity::Column::LastUsedAt, Expr::value(Some(now)))
            .filter(entity::Column::Id.eq(db_id(id)?))
            .filter(entity::Column::Digest.eq(digest))
            .filter(Condition::any().add(entity::Column::ExpiresAt.is_null()).add(entity::Column::ExpiresAt.gt(now)))
            .exec(&self.0)
            .await?;
        Ok(result.rows_affected == 1)
    }

    async fn revoke(&self, owner: &i64, id: u64) -> Result<bool, DbErr> {
        let result = entity::Entity::delete_many()
            .filter(entity::Column::Id.eq(db_id(id)?))
            .filter(entity::Column::OwnerId.eq(*owner))
            .exec(&self.0)
            .await?;
        Ok(result.rows_affected == 1)
    }

    async fn list(&self, owner: &i64) -> Result<Vec<TokenMetadata>, DbErr> {
        entity::Entity::find()
            .filter(entity::Column::OwnerId.eq(*owner))
            .all(&self.0)
            .await?
            .into_iter()
            .map(metadata)
            .collect()
    }
}
