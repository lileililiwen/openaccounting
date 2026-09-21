# Proposal: Production hardening and disclosure readiness

## Why

Backups are documented as manual `pg_dump` + tar with a scheduled local backup worker, but there is no S3 backup target, no PITR story, and no stated RTO/RPO. SQLite backup parity is missing. Observability stops at `/healthz` `/readyz` `/metrics` with no tracing. Releases are reproducible and cosign-signed but publish no SBOM attestation. `SECURITY.md` still contains `[INSERT SECURITY EMAIL]`, and rate limiting covers login only. None of this is shippable as professional self-hosted software.

## What Changes

- Real security contact plus disclosure SLA, threat-model doc, and log-redaction policy.
- Scheduled backups gain S3 target, retention enforcement, PITR guide, stated RTO/RPO, and SQLite parity.
- OTel tracing behind an opt-in flag with redacted spans.
- SBOM + provenance attestation on releases.
- Global rate limits on auth, API, webhook, and import endpoints.

## Capabilities

### New Capabilities
- `ops-hardening`: disclosure process, S3/PITR backups, RTO/RPO, tracing, SBOM attestation, rate limits.

### Modified Capabilities
- `scheduled-backups`: gains S3 target and retention verification without changing cron semantics.
- `metrics`: gains trace-correlated request IDs without changing Prometheus format.
- `operations-security`: replaces placeholder contact with an operative disclosure process.

## Impact

Affected: `SECURITY.md`, backup workers, release workflow, observability middleware, `docs/production-deployment.md`, `docs/backup-restore.md`. Unaffected: accounting math, report content, API shapes.
