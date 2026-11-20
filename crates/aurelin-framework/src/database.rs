use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::{MigrationStatus, MigratorTrait};

pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut options = ConnectOptions::new(url);
    options.max_connections(if url.contains(":memory:") { 1 } else { 5 });
    Database::connect(options).await
}

pub async fn migrate<M: MigratorTrait>(database: &DatabaseConnection) -> Result<usize, DbErr> {
    let pending = M::get_pending_migrations(database).await?.len();
    M::up(database, None).await?;
    Ok(pending)
}

pub async fn migration_status<M: MigratorTrait>(
    database: &DatabaseConnection,
) -> Result<Vec<(String, MigrationStatus)>, DbErr> {
    Ok(M::get_migration_with_status(database)
        .await?
        .into_iter()
        .map(|migration| (migration.name().to_owned(), migration.status()))
        .collect())
}

pub async fn rollback<M: MigratorTrait>(database: &DatabaseConnection) -> Result<(), DbErr> {
    M::down(database, Some(1)).await
}

pub async fn fresh<M: MigratorTrait>(database: &DatabaseConnection) -> Result<(), DbErr> {
    M::fresh(database).await
}
