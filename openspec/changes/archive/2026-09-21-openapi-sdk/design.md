## Context

API routers exist per resource with bearer `oa_live_` tokens and problem-JSON errors. Pagination helpers and `next_link` exist. Jobs cover digests, invoice reminders, recurring invoices, template runs, webhook delivery, email send. No machine-readable contract is published.

## Goals / Non-Goals

**Goals:**
- Every web mutation has an API equivalent with identical authorization.
- Retries are safe via idempotency keys.

**Non-Goals:**
- Versioned `/api/v2` in this change (complete v1 coverage first).
- A plugin sandbox/JS runtime (rules are declarative event/condition/action only).

## Decisions

- **Hand-maintained OpenAPI YAML generated from route inventory tests, not proc-macro annotations.** WHY: annotations drift across 13 routers; an inventory test fails when a route lacks a doc entry, keeping docs honest.
- **Idempotency via `Idempotency-Key` header on POST, stored with response hash for 24h.** WHY: matches existing `0052_api_idempotency` table intent; 24h bounds storage while covering client retries.
- **Incoming events as signed POST intakes mapped to internal job kinds.** WHY: reuses the scheduler queue and retry/backoff instead of a second execution path.
- **Rule builder limited to allowlisted triggers and actions (invoice created/overdue, transaction posted → webhook/email/categorize).** WHY: safe subset first; arbitrary code execution is explicitly out.
- **No new HTTP framework or SDK language commitment.** WHY: OpenAPI + curl examples unblock all languages; official SDKs follow adoption.

## Risks / Trade-offs

- Doc drift as routes evolve → Mitigation: route-inventory test fails CI when OpenAPI misses a route.
- Idempotency storage growth → Mitigation: 24h TTL with scheduled prune worker.
- Rule-action abuse (email spam) → Mitigation: per-ledger rate limits and owner-only rule management.
