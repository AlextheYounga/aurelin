---
name: aurelin-data-and-jobs
description: Change an Aurelin application's database entities, migrations, transactions, background jobs, queues, workers, or scheduled tasks.
---

# Aurelin data and background work

Read `../../docs/aurelin.md` and inspect the existing database, entity, job,
schedule, and test modules before editing.

1. Use SeaORM for application persistence and SeaQuery/SeaORM migrations for
   schema changes. Keep migrations and entity definitions aligned.
2. Put schema changes in `database/migrations/`; never rely on startup or entity
   declarations to mutate the schema automatically.
3. Keep transactions around one coherent unit of work and propagate failures so
   partial writes are not committed.
4. Make jobs serializable, retry-safe, and independent of request-local state.
   Assume workers run in another process and may execute a job more than once.
5. Register scheduled work in `src/schedule.rs` and use the project's lock
   facilities when overlapping execution would be unsafe.
6. Exercise observable persistence or worker behavior in an integration test.
   Run migrations explicitly in fixtures that require schema.
