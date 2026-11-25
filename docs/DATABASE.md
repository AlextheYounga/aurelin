# Database development (current capabilities)

Generated applications use SeaORM 1.1 directly, not an Eloquent compatibility
layer. SQLite is the configured driver. PostgreSQL and MySQL have not been
qualified by Aurelin's generated-application tests.

## Entities and relations

In authenticated projects, application entities live in `src/entities/`.
Declare the table and primary key with SeaORM's entity derives; set
`auto_increment = false` for a key supplied by the application. Match nullable
columns with `Option<T>`, and use SeaORM's `DeriveActiveEnum` for typed enum
values where appropriate. Define relations and foreign keys explicitly; the
database migration must actually create the corresponding constraints. SeaORM
does not automatically maintain Eloquent's `created_at` and `updated_at`:
choose column types and explicitly set them in application writes or database
defaults. Never deserialize untrusted request data directly into an active
model without explicitly selecting allowed fields.

Use SeaORM's relation queries, including `find_with_related` or the data loader,
to fetch related rows for collections rather than querying once per parent.
Check the resulting query count and ordering for your workload. Use SeaORM's
paginator or cursor APIs directly and specify a stable ordering; no Aurelin
pagination response convention is established yet. SeaORM's `DbErr` and
transaction errors should be preserved until a boundary can meaningfully
handle them.

## Migrations

Add application migrations under `database/migrations/`, declare each module in
`database/migrations/mod.rs`, and add it to `Migrator::migrations()` in dependency
order. Implement both `up` and `down`. Migration registration is explicit, not
discovered by filename. Existing commands are `cargo aurelin migrate`,
`migrate:status`, `migrate:rollback`, and `migrate:fresh`. Rollback reverts one
migration, not a Laravel batch. Fresh drops all tables in the selected database;
do not use it against a shared database. Migration-changing commands require
`--force` when `APP_ENV=production`.

`aurelin new` prints the command to migrate; it does not run migrations. The
authenticated starter rejects pending migrations for both file-backed and
in-memory databases; startup never applies them. SeaORM's migration status check
may create its bookkeeping table. Tests that open a separate application
connection should explicitly migrate a temporary file-backed SQLite database and
keep the temporary directory alive for the application's lifetime.

## Transactions and test data

Use `TransactionTrait::transaction` for multi-write operations: returning `Ok`
commits and returning `Err` rolls back. Pass the provided transaction to every
query in that operation. Alternatively use `begin`, then explicitly `commit`
or `rollback`; an uncommitted transaction is rolled back on drop. Avoid external
side effects inside a transaction if they cannot be rolled back.

Factories and seeders are not yet provided by generated applications. There is
no `db:seed` command or registration seam. Applications can define their own
typed constructors and seed operations using SeaORM and `fake`, with an explicit
seeded random generator where reproducibility matters. Do not assume fake data
is unique or safe for production.

References: Laravel 13 documentation for [models](https://laravel.com/docs/13.x/eloquent),
[relationships](https://laravel.com/docs/13.x/eloquent-relationships),
[migrations](https://laravel.com/docs/13.x/migrations), and
[seeding](https://laravel.com/docs/13.x/seeding); SeaORM 1.1 documentation for
[generation](https://www.sea-ql.org/SeaORM/docs/1.1.x/generate-entity/sea-orm-cli/),
[relations](https://www.sea-ql.org/SeaORM/docs/1.1.x/relation/data-loader/), and
[transactions](https://www.sea-ql.org/SeaORM/docs/1.1.x/advanced-query/transaction/).
