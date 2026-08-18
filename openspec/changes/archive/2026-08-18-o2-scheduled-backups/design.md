# ## Context

Manual backups exist but no automation.

## Goals / Non-Goals

**Goals:**
- Hands-off daily backup with bounded retention.

**Non-Goals:**
- Differential / incremental backups.

## Decisions

- `cron` crate for tick detection.
- `pg_dump` is the canonical Postgres tool; output piped through `gzip`.
- S3 support gated behind a feature flag.
