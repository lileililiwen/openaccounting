# Design — automation-platform

## Context

Workers today are ad-hoc tokio tasks (`src/workers/`: bank-feed sync,
login prune, audit anchor, backups). Recurring templates, reminders,
and webhook deliveries need the same plumbing: "run this later,
reliably, maybe more than once."

## Goals / Non-Goals

**Goals:**
- One queue, many job kinds; no Redis; survives restarts.
- Idempotency at the business level (template runs, reminders).

**Non-Goals:**
- Distributed cron semantics beyond SKIP LOCKED claiming.
- Webhook fan-out to >10k subscribers (SMB scale).

## Decisions

- **Postgres `jobs` table + `FOR UPDATE SKIP LOCKED`**, not a new
  broker. WHY: zero new infrastructure, matches tower-sessions/
  sqlx posture of the repo. Alternatives considered: `apalis` crate —
  rejected (extra dependency for what is ~200 lines here); Redis/BullMQ —
  rejected (new runtime component).
- **Events emitted from service layer, not handlers.** WHY: CLI import
  and API writes must trigger webhooks too; handlers are one of three
  write paths.
- **HMAC per subscription secret, raw-body signing.** WHY: mirrors
  Plaid/GitHub conventions receivers already know; body bytes are what
  arrives, so sign exactly those.
- **Email via `lettre` with SMTP_URL config.** Alternative considered:
  hand-rolled SMTP — rejected (unsafe to get TLS right); API-provider-
  only (Postmark) — rejected (self-hosted users often have no SaaS
  egress).

## Risks / Trade-offs

- Queue table growth → Mitigation: purge delivered jobs older than 30
  days in the existing nightly prune worker.
- Duplicate template posts on clock skew → Mitigation: unique index on
  `(template_id, due_date)` in posted-transactions metadata.
- Webhook SSRF → Mitigation: HTTPS-only targets, no private-IP
  resolution at delivery time, response body capped at 4 KB.
- SMTP misconfiguration delaying jobs → Mitigation: email sends are
  their own jobs; a dead letter never blocks template posting.

## Migration Plan

1. `jobs` migration + runner skeleton behind existing worker spawn.
2. Move `process_due` logic into a job handler (route keeps working).
3. Add notification channels + preferences columns.
4. Add webhook tables + event emission points.
