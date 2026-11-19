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
                    .col(text_null(Users::TwoFactorSecret))
                    .col(text_null(Users::TwoFactorRecoveryCodes))
                    .col(big_integer_null(Users::TwoFactorConfirmedAt))
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
                    .table(Passkeys::Table)
                    .if_not_exists()
                    .col(pk_auto(Passkeys::Id))
                    .col(big_integer(Passkeys::UserId))
                    .col(string(Passkeys::Name))
                    .col(string_uniq(Passkeys::CredentialId))
                    .col(text(Passkeys::Credential))
                    .col(big_integer_null(Passkeys::LastUsedAt))
                    .col(big_integer(Passkeys::CreatedAt))
                    .col(big_integer(Passkeys::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Passkeys::Table, Passkeys::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Passkeys::Table).to_owned())
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
    TwoFactorSecret,
    TwoFactorRecoveryCodes,
    TwoFactorConfirmedAt,
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
enum Passkeys {
    Table,
    Id,
    UserId,
    Name,
    CredentialId,
    Credential,
    LastUsedAt,
    CreatedAt,
    UpdatedAt,
}
