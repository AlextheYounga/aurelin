use std::error::Error;

use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder};

use crate::{entities::user, factory};

/// Explicit application seeder registration, executed in order.
pub async fn run(database_url: &str) -> Result<(), Box<dyn Error>> {
    let database = crate::database::connect(database_url).await?;
    example_post(&database).await?;
    Ok(())
}

async fn example_post(database: &sea_orm::DatabaseConnection) -> Result<(), Box<dyn Error>> {
    let user = user::Entity::find()
        .order_by_asc(user::Column::Id)
        .one(database)
        .await?
        .ok_or("example post seeder requires an existing user")?;
    factory::post(user.id, 42).insert(database).await?;
    println!("Seeded an example post for user {}.", user.id);
    Ok(())
}
