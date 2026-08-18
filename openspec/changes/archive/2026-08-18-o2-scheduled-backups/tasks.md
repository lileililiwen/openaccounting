## 1. Testing

- [x] 1.1 Worker: `backup_worker_creates_tarball` (in-memory).
- [x] 1.2 Worker: `backup_worker_retention_deletes_old`.
- [x] 1.3 Worker: `backup_worker_failure_records_error`.
- [x] 1.4 HTTP: `http_admin_backups_schedule_lists_runs` + `http_non_admin_blocked_from_schedule`.

## 2. Implementation

- [x] 2.1 `Cargo.toml` — `cron = "0.12"`. (`aws-sdk-s3` not added — the proposal marked it optional and S3 destination is out of scope for this iteration.)
- [x] 2.2 `src/workers/backup.rs` — `BackupConfig`, `run_backup_with_pool`, `prune_for_test`, `spawn_backup_worker`. Cron ticks every 30 s; on each tick inserts a `backup_runs` row, calls `run_backup`, prunes.
- [x] 2.3 `src/handlers/admin_backups_schedule.rs` — `GET /admin/backups/schedule` returns JSON `{cron, keep, dir, runs: [...20]}`.
- [x] 2.4 Env vars: `BACKUP_CRON` (default `0 0 2 * * * *`), `BACKUP_KEEP` (default 7), `BACKUP_DIR` (default `./data/backups`). The cron crate uses seven-field format (sec min hr dom mon dow yr) — the proposal's classic five-field expression is rejected by this parser, so the default is documented in `DEFAULT_BACKUP_CRON`.
- [x] 2.5 `migrations/0038_add_backup_runs.sql` — `backup_runs(id, kind, scheduled_for, started_at, finished_at, status, filename, size_bytes, error)` with indexes.
- [x] 2.6 `src/lib.rs` — `workers::backup::spawn_backup_worker(state, BackupConfig::from_env())` spawned alongside the bank-feed / audit-anchor workers.
- [x] `Cargo.toml` — `tempfile` promoted to a regular dep (was test-only).

## 3. Validation

- [x] 3.1 `openspec validate o2-scheduled-backups` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support` — 4 unit + 5 integration tests pass; full integration suite 185 / 185.
- [ ] 3.5 `openspec archive o2-scheduled-backups`.
