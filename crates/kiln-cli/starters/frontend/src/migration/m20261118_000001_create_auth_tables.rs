use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(pk_auto(Users::Id))
                    .col(string(Users::Name))
                    .col(string_uniq(Users::Email))
                    .col(big_integer_null(Users::EmailVerifiedAt))
                    .col(string(Users::PasswordHash))
                    .col(string_null(Users::RememberToken))
                    .col(big_integer(Users::CreatedAt))
                    .col(big_integer(Users::UpdatedAt))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(PasswordResetTokens::Table)
                    .if_not_exists()
                    .col(string(PasswordResetTokens::Email).primary_key())
                    .col(string(PasswordResetTokens::TokenHash))
                    .col(big_integer(PasswordResetTokens::CreatedAt))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(EmailVerificationTokens::Table)
                    .if_not_exists()
                    .col(big_integer(EmailVerificationTokens::UserId).primary_key())
                    .col(string(EmailVerificationTokens::TokenHash))
                    .col(big_integer(EmailVerificationTokens::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                EmailVerificationTokens::Table,
                                EmailVerificationTokens::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(EmailVerificationTokens::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(PasswordResetTokens::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Name,
    Email,
    EmailVerifiedAt,
    PasswordHash,
    RememberToken,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PasswordResetTokens {
    Table,
    Email,
    TokenHash,
    CreatedAt,
}

#[derive(DeriveIden)]
enum EmailVerificationTokens {
    Table,
    UserId,
    TokenHash,
    CreatedAt,
}
