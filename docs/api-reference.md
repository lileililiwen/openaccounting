# API reference pointer

The machine API lives under `/api/v1` and is bearer-token authenticated.
This page points at the contract; the per-resource routers in
`src/api/` are authoritative for shapes.

## Base and auth

- Base path: `/api/v1` (see `src/api/mod.rs`).
- Auth: bearer token on every route via the `require_bearer` middleware.
  Mint and manage tokens as described in `README.md`.
- Errors use the problem shape in `src/api/problem.rs`.

## Resources

| Prefix | Router |
| --- | --- |
| ledgers | `src/api/ledgers.rs` |
| accounts | `src/api/accounts.rs` |
| transactions | `src/api/transactions.rs` |
| invoices | `src/api/invoices.rs` |
| payments | `src/api/payments.rs` |
| contacts | `src/api/contacts.rs` |
| documents | `src/api/documents.rs` |
| budgets | `src/api/budgets.rs` |
| bank feeds | `src/api/bank_feeds.rs` |
| reports | `src/api/reports.rs` |

## Operational endpoints

- `GET /healthz` — liveness.
- `GET /readyz` — readiness (database reachable).
- `GET /metrics` — Prometheus metrics (enable with `METRICS_ENABLED`,
  see `docs/production-deployment.md`).

## Idempotency

Write idempotency-key semantics are documented alongside the
`openapi-sdk` package when it lands; until then, clients must treat
retries as potentially duplicate and reconcile via list endpoints.
