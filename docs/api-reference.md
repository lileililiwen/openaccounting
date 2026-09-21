# API reference pointer

The machine API lives under `/api/v1` and is bearer-token authenticated.
The canonical contract is the OpenAPI 3.1 document served at
`GET /api/openapi.yaml` (see `docs/openapi.yaml`).

## Base and auth

- Base path: `/api/v1` (see `src/api/mod.rs`).
- Auth: bearer token on every route via the `require_bearer` middleware.
  Mint and manage tokens as described in `README.md`.
- Errors use the problem shape in `src/api/problem.rs`.

## Resources

| Prefix | Router | Operations |
| --- | --- | --- |
| ledgers | `src/api/ledgers.rs` | list, create, get, patch |
| accounts | `src/api/accounts.rs` | list, create, get, patch |
| transactions | `src/api/transactions.rs` | list, create, get, reverse |
| invoices | `src/api/invoices.rs` | list, create, get, mark-paid, void |
| payments | `src/api/payments.rs` | list, create, get |
| contacts | `src/api/contacts.rs` | list, create, get, patch |
| documents | `src/api/documents.rs` | list, upload, download, delete |
| budgets | `src/api/budgets.rs` | list, create, get, delete |
| bank feeds | `src/api/bank_feeds.rs` | list, get |
| reports | `src/api/reports.rs` | trial-balance, balance-sheet, income-statement, cash-flow, general-ledger |

## Idempotency

All `POST` endpoints accept an `Idempotency-Key` header. Replays within
24 hours return the original response without re-executing the mutation.
Keys are scoped per API token + ledger. A key reuse with a different
request body is a hard `422` (`/errors/idempotency-conflict`).

Expired records are pruned nightly by the scheduler.

## Incoming Events

External systems push signed events to:

```
POST /api/events/{ledger_id}
X-OA-Event-Signature: sha256=<hmac-hex>
Content-Type: application/json

{"type": "external.sync", "data": {...}}
```

The ledger must have an `incoming_events_secret` (generate via
`/ledgers/{id}/automations/rotate-secret` in the web UI). Allowlisted
types: `external.sync`, `external.document.received`,
`external.payment.received`. Events are routed to the automation rule
engine and executed on the jobs queue with retry/backoff.

## Automation Rules

Ledger owners manage rules via the web UI at
`/ledgers/{id}/automations`. Each rule maps:

- **Trigger** — an event type (e.g. `transaction.posted`, `invoice.overdue`)
- **Conditions** — a JSON object (optional): `description_contains`, `amount_gte`, `amount_lte`, `days_past_due_gte`
- **Action** — `webhook_post` (config: `{subscription_id}`), `email_notify` (config: `{to}`), or `categorize_transaction` (config: `{cost_center}`)

Rules execute asynchronously on the jobs queue. Per-ledger rate limit:
120 action jobs per hour.

## Operational endpoints

- `GET /healthz` — liveness.
- `GET /readyz` — readiness (database reachable).
- `GET /metrics` — Prometheus metrics (enable with `METRICS_ENABLED`,
  see `docs/production-deployment.md`).
- `GET /api/openapi.yaml` — OpenAPI 3.1 specification (public, no auth).
