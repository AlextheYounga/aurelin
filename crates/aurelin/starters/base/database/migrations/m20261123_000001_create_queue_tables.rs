use sea_orm_migration::{prelude::*, schema::*};

// Application-owned schema required by the pinned apalis-sqlite 1.1.1 backend.

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Workers::Table)
                    .if_not_exists()
                    .col(string_uniq(Workers::Id))
                    .col(string(Workers::WorkerType))
                    .col(string(Workers::StorageName))
                    .col(text_null(Workers::Layers))
                    .col(
                        ColumnDef::new(Workers::LastSeen)
                            .big_integer()
                            .not_null()
                            .default(Expr::cust("(strftime('%s', 'now'))")),
                    )
                    .col(big_integer_null(Workers::StartedAt))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Jobs::Table)
                    .if_not_exists()
                    .col(binary(Jobs::Job))
                    .col(string_uniq(Jobs::Id).primary_key())
                    .col(string(Jobs::JobType))
                    .col(string(Jobs::Status).default("Pending"))
                    .col(integer(Jobs::Attempts).default(0))
                    .col(integer(Jobs::MaxAttempts).default(25))
                    .col(
                        ColumnDef::new(Jobs::RunAt)
                            .big_integer()
                            .not_null()
                            .default(Expr::cust("(strftime('%s', 'now'))")),
                    )
                    .col(text_null(Jobs::LastResult))
                    .col(big_integer_null(Jobs::LockAt))
                    .col(string_null(Jobs::LockBy))
                    .col(big_integer_null(Jobs::DoneAt))
                    .col(integer(Jobs::Priority).default(0))
                    .col(text_null(Jobs::Metadata))
                    .col(string_null(Jobs::IdempotencyKey))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Jobs::Table, Jobs::LockBy)
                            .to(Workers::Table, Workers::Id),
                    )
                    .to_owned(),
            )
            .await?;

        for index in queue_indexes() {
            manager.create_index(index).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Jobs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Workers::Table).to_owned())
            .await
    }
}

fn queue_indexes() -> Vec<IndexCreateStatement> {
    vec![
        index("idx_workers_type", Workers::Table, [Workers::WorkerType]),
        index("idx_workers_last_seen", Workers::Table, [Workers::LastSeen]),
        index("idx_jobs_lock_by", Jobs::Table, [Jobs::LockBy]),
        index(
            "idx_jobs_type_status_run_at",
            Jobs::Table,
            [Jobs::JobType, Jobs::Status, Jobs::RunAt],
        ),
        index(
            "idx_jobs_status_run_at",
            Jobs::Table,
            [Jobs::Status, Jobs::RunAt, Jobs::Attempts],
        ),
        index(
            "idx_jobs_pending",
            Jobs::Table,
            [Jobs::JobType, Jobs::Priority, Jobs::RunAt, Jobs::Id],
        ),
        index(
            "idx_jobs_recent",
            Jobs::Table,
            [Jobs::Status, Jobs::DoneAt, Jobs::RunAt],
        ),
        Index::create()
            .name("idx_jobs_idempotency_key")
            .table(Jobs::Table)
            .col(Jobs::JobType)
            .col(Jobs::IdempotencyKey)
            .unique()
            .to_owned(),
    ]
}

fn index<T, C, const N: usize>(name: &str, table: T, columns: [C; N]) -> IndexCreateStatement
where
    T: IntoIden + 'static,
    C: IntoIden + 'static,
{
    let mut index = Index::create();
    index.name(name).table(table);
    for column in columns {
        index.col(column);
    }
    index.to_owned()
}

#[derive(DeriveIden)]
enum Workers {
    Table,
    Id,
    WorkerType,
    StorageName,
    Layers,
    LastSeen,
    StartedAt,
}

#[derive(DeriveIden)]
enum Jobs {
    Table,
    Job,
    Id,
    JobType,
    Status,
    Attempts,
    MaxAttempts,
    RunAt,
    LastResult,
    LockAt,
    LockBy,
    DoneAt,
    Priority,
    Metadata,
    IdempotencyKey,
}
