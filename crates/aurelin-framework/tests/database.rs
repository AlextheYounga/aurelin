use std::error::Error;

use aurelin_framework::database;
use sea_orm_migration::MigrationStatus;
use sea_orm_migration::prelude::*;

struct TestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateWidgets)]
    }
}

#[derive(DeriveMigrationName)]
struct CreateWidgets;

#[async_trait::async_trait]
impl MigrationTrait for CreateWidgets {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Widgets::Table)
                    .col(ColumnDef::new(Widgets::Id).integer().not_null().primary_key())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Widgets::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Widgets {
    Table,
    Id,
}

#[tokio::test]
async fn migration_commands_report_apply_rollback_and_rebuild_application_schema() -> Result<(), Box<dyn Error>> {
    let connection = database::connect("sqlite::memory:").await?;

    let status = database::migration_status::<TestMigrator>(&connection).await?;
    assert_eq!(status[0].1, MigrationStatus::Pending);

    assert_eq!(database::migrate::<TestMigrator>(&connection).await?, 1);
    assert_eq!(database::migrate::<TestMigrator>(&connection).await?, 0);
    let status = database::migration_status::<TestMigrator>(&connection).await?;
    assert_eq!(status[0].1, MigrationStatus::Applied);

    database::rollback::<TestMigrator>(&connection).await?;
    let status = database::migration_status::<TestMigrator>(&connection).await?;
    assert_eq!(status[0].1, MigrationStatus::Pending);

    database::fresh::<TestMigrator>(&connection).await?;
    let status = database::migration_status::<TestMigrator>(&connection).await?;
    assert_eq!(status[0].1, MigrationStatus::Applied);
    Ok(())
}
