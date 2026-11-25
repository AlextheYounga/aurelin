use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tokens::Table)
                    .col(pk_auto(Tokens::Id))
                    .col(big_integer(Tokens::UserId))
                    .col(string(Tokens::Name))
                    .col(string_uniq(Tokens::Digest))
                    .col(json(Tokens::Abilities))
                    .col(big_integer_null(Tokens::ExpiresAt))
                    .col(big_integer_null(Tokens::LastUsedAt))
                    .col(big_integer(Tokens::CreatedAt))
                    .col(big_integer(Tokens::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Tokens::Table, Tokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("personal_access_tokens_user_id_idx")
                    .table(Tokens::Table)
                    .col(Tokens::UserId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("personal_access_tokens_expires_at_idx")
                    .table(Tokens::Table)
                    .col(Tokens::ExpiresAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Tokens::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Tokens {
    #[sea_orm(iden = "personal_access_tokens")]
    Table,
    Id,
    UserId,
    Name,
    Digest,
    Abilities,
    ExpiresAt,
    LastUsedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
