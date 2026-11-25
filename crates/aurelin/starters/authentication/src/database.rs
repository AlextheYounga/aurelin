use std::error::Error;

use aurelin_framework::database;
use sea_orm::{DatabaseConnection, DbErr};
use sea_orm_migration::MigrationStatus;

use crate::migrations::Migrator;

pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let database = database::connect(url).await?;
    if database::migration_status::<Migrator>(&database)
        .await?
        .iter()
        .any(|(_, status)| *status == MigrationStatus::Pending)
    {
        return Err(DbErr::Custom(
            "database migrations are pending; run `cargo aurelin migrate`".into(),
        ));
    }
    Ok(database)
}

pub async fn migrate(url: &str) -> Result<(), Box<dyn Error>> {
    let database = database::connect(url).await?;
    let pending = database::migrate::<Migrator>(&database).await?;
    println!("Applied {pending} migration(s).");
    Ok(())
}

pub async fn migration_status(url: &str) -> Result<(), Box<dyn Error>> {
    let database = database::connect(url).await?;
    for (name, status) in database::migration_status::<Migrator>(&database).await? {
        println!("{status:<10} {name}");
    }
    Ok(())
}

pub async fn rollback(url: &str) -> Result<(), Box<dyn Error>> {
    let database = database::connect(url).await?;
    database::rollback::<Migrator>(&database).await?;
    println!("Rolled back the latest migration.");
    Ok(())
}

pub async fn fresh(url: &str) -> Result<(), Box<dyn Error>> {
    let database = database::connect(url).await?;
    database::fresh::<Migrator>(&database).await?;
    println!("Dropped all tables and applied all migrations.");
    Ok(())
}
