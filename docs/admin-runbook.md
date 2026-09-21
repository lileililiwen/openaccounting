# Admin runbook

Day-to-day operations for a self-hosted OpenAccounting instance.
Deployment prerequisites and environment variables live in
`docs/production-deployment.md`; backup and restore steps live in
`docs/backup-restore.md`.

## Health checks

- Liveness: `GET /healthz` must return 200.
- Readiness: `GET /readyz` must return 200 (fails when the database
  is unreachable — point the load balancer here).
- Metrics: `GET /metrics` (Prometheus format).

## Start, stop, upgrade

1. Stop the old binary (keep the reverse proxy running).
2. Back up the database (`docs/backup-restore.md`).
3. Deploy the new binary and run `cargo sqlx migrate run`
   (equivalently `sqlx migrate run --source migrations`).
4. Start the binary and confirm `/readyz` returns 200.
5. Rollback: stop, restore the backup, redeploy the previous
   binary. Every migration is reversible — see the
   `migrations-reversible` CI job and `scripts/migrate-down.sh`.

## Data model

The entity graph is rendered in `docs/erd.md` (generated from
`migrations/*.sql` — regenerate after any migration change).

## User and access administration

- First user registers via the web UI; further users are invited
  per ledger from the ledger share page.
- Global admin role is managed via the admin routes
  (`src/handlers/admin.rs`); review `CODEOWNERS` for who holds it.
- Sessions expire per the session-timeout policy; signed cookies
  are validated on every request.

## Incident basics

1. Check `/readyz` and database connectivity first.
2. Check disk space for `DOCUMENTS_DIR` and the database volume.
3. Collect logs (`RUST_LOG=info,openaccounting=debug,sqlx=warn`)
   and open an issue with the bug template
   (`.github/ISSUE_TEMPLATE/bug_report.md`).
4. Never paste `APP_SECRET`, tokens, or user data into issues.
