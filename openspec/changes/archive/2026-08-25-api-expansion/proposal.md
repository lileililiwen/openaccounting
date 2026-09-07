# API v2 Coverage: Full Resource Surface, Durable Idempotency, Pagination

## Why

`/api/v1` covers only ledgers, accounts, transactions, and four
reports (`src/api/`). Invoices, contacts, payments, documents,
budgets, and bank feeds are UI-only — yet every comparable product
exposes them: Firefly III's full JSON API, Invoice Ninja's OpenAPI
surface, Bigcapital, Zoho (Standard+), FreshBooks. Two defects make
the current API unreliable for automation: the `Idempotency-Key`
cache is a per-process HashMap lost on restart
(`src/api/transactions.rs:76`), and the cash-flow endpoint is a
"Minimal stub" (`src/api/reports.rs:196`) while the HTML version is
complete.

## What Changes

- `/api/v1` gains read + write endpoints for invoices, payments,
  contacts, documents (metadata + upload/download), budgets, and
  bank-feed links (read) — same bearer-token auth and RFC 7807 errors.
- Idempotency keys persist in Postgres with stored response replay
  (24 h window).
- Cursor pagination standard (`?cursor=&limit=`, `Link: rel="next"`).
- Per-token rate limit (default 120 req/min, 429 + `Retry-After`).
- Cash-flow endpoint returns the full report the UI already computes.

## Capabilities

### New Capabilities

- `api-v2-coverage`: complete resource surface, durable idempotency,
  cursor pagination, per-token rate limits.

## Impact

**New files:** `src/api/{invoices,payments,contacts,documents,budgets,bank_feeds}.rs`,
`migrations/00xx_api_idempotency.sql`.
**Modified:** `src/api/mod.rs` (router, middleware), `src/api/reports.rs`
(cash-flow), token auth layer (rate limiter).
