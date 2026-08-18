# Scheduled Backups

## Why

`src/handlers/backups.rs` supports manual backup creation
(`/admin/backups/create`). Nothing schedules it. Operators must run cron
themselves, or rely on Postgres-level pg_dump jobs. Firefly III bundles
`backup:run` on a schedule; Akaunting has a similar Laravel
`backup:run`.

## What Changes

- New `backup_schedule` config (env var): cron expression, retention
  count, destination (local FS / S3 / both).
- A tokio worker runs every minute; when the cron ticks, it spawns a
  `pg_dump`, tars the documents dir, and writes to the destination.
- Retention: keep the latest N backups; delete older.
- A small REST endpoint shows the schedule and recent runs.

## Capabilities

### New Capabilities

- `scheduled-backups`: Cron-driven backup worker.

## Impact

**New files:**
- `src/workers/backup.rs`.
- `src/handlers/admin_backups_schedule.rs`.
- `tests/integration/scheduled_backup.rs`.

**Modified files:**
- `src/config.rs` — backup env vars.
- `Cargo.toml` — `cron = "0.12"`, optional `aws-sdk-s3`.
