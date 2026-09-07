# Design — api-expansion

## Context

`src/api/` already establishes the patterns: bearer tokens
(Argon2id-hashed), RFC 7807 errors, role checks. This change extends
coverage and fixes two reliability defects rather than inventing new
conventions.

## Goals / Non-Goals

**Goals:**
- UI/API parity for the resources integrators ask for first (AR).
- Automation-safe semantics: idempotency + pagination + limits.

**Non-Goals:**
- GraphQL, bulk JSON:API, or OAuth2 authorization-code flows.
- Webhook emission (separate `outgoing-webhooks` capability).

## Decisions

- **Persist idempotency in Postgres keyed by `(key, token_id)`.**
  WHY: the HashMap dies on restart and lies under multi-instance
  deployment; a table is one migration. Alternative considered:
  `moka` cache with disk spill — rejected (still per-process).
- **Signed opaque cursors (HMAC over last-seen sort key).**
  WHY: stateless, no cursor table; offset pagination breaks when rows
  are inserted between pages.
- **Rate limit in a tower middleware with an in-memory sliding window
  per token.** WHY: SMB scale makes cross-node sync unnecessary;
  documented as single-node semantics. Alternative considered: Redis —
  rejected (new infra for no current need).
- **Documents upload via multipart to the existing storage layer**
  (`src/storage/`, S3-capable). WHY: reuse validation/AV-scan path from
  `src/upload.rs`; API uploads must not bypass it.

## Risks / Trade-offs

- Large document uploads through the API → Mitigation: enforce the same
  size/MIME caps as the web uploader and stream to storage.
- Rate limiter memory growth → Mitigation: fixed-size LRU keyed by token id.
- Replay window abuse → Mitigation: response bodies capped at 1 MB for
  replay storage; larger responses replay only status + Location.

## Migration Plan

1. Idempotency migration; swap HashMap for table-backed store.
2. Pagination helper + retrofit existing list endpoints.
3. New resource modules behind the existing auth layer.
4. Cash-flow endpoint delegates to the HTML report's service function.
