## Context

`src/workers/backup.rs` and `src/workers/scheduler.rs` run scheduled tasks. Storage already abstracts filesystem vs S3 (`s3-storage` spec). Metrics use `metrics` + Prometheus exporter. Release workflow signs artifacts with cosign. Login rate limiting and prune workers exist.

## Goals / Non-Goals

**Goals:**
- A disclosed vulnerability reaches a human within 48h.
- A destroyed server is recoverable to a stated RPO via tested procedure.

**Non-Goals:**
- Multi-region HA/failover (single-tenant backup/restore only).
- SOC 2 certification (controls documented, audit explicitly out).

## Decisions

- **S3 backup target reuses the existing `Storage` trait.** WHY: filesystem and S3 already share put/get/delete; backups become another caller rather than a parallel implementation. Alternative considered: shell out to `wal-g` — rejected: new binary dependency breaks single-binary ops.
- **PITR via base dump + WAL archive guide, not continuous replication management.** WHY: self-hosters can follow pg_basebackup + archive_command docs; operating replicas for them is out of scope.
- **OTel tracing with `tracing-opentelemetry` behind `TRACING_ENABLED=false` default.** WHY: opt-in avoids overhead and exporter deps for small deploys. Alternative considered: custom span exporter — rejected: OTel is the standard collectors speak.
- **SBOM via `cargo auditable` + release provenance attestation.** WHY: no new manifest format; auditors get artifact-to-source linkage with cosign.
- **Token-bucket rate limits in middleware with per-route budgets.** WHY: one mechanism covers auth, API, webhooks, imports without per-handler code.

## Risks / Trade-offs

- S3 credential handling widens secret surface → Mitigation: reuse existing S3 env config; backups never log credentials.
- Tracing span cardinality → Mitigation: redact IDs by default; sample tail-heavy routes.
- SBOM generation slows release → Mitigation: SBOM built once per tag in the release job only.
