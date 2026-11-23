use std::error::Error;

use aurelin_framework::database;

use crate::migrations::Migrator;

pub async fn migrate(url: &str) -> Result<(), Box<dyn Error>> {
    let database = database::connect(url).await?;
    let pending = database::migrate::<Migrator>(&database).await?;
    println!("Applied {pending} migration(s).");
    Ok(())
}

pub async fn migration_status(url: &str) -> Result<(), Box<dyn Error>> {
    let database = database::connect(url).await?;
    let migrations = database::migration_status::<Migrator>(&database).await?;
    if migrations.is_empty() {
        println!("No migrations registered.");
    }
    for (name, status) in migrations {
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
