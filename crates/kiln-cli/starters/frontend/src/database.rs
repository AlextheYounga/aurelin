use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

use crate::migration::Migrator;

pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut options = ConnectOptions::new(url);
    options.max_connections(if url.contains(":memory:") { 1 } else { 5 });
    let database = Database::connect(options).await?;
    Migrator::up(&database, None).await?;
    Ok(database)
}
