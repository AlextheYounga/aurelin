use sea_orm_migration::prelude::*;

mod m20261123_000001_create_queue_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20261123_000001_create_queue_tables::Migration)]
    }
}
