# api-v2-coverage Specification (delta)

## ADDED Requirements

### Requirement: Resource Endpoints

The `/api/v1` surface SHALL include, per ledger and subject to the
existing role rules (owner/editor write, viewer read):

| Resource | Methods |
|---|---|
| `invoices`, `invoices/{id}` | GET list/show, POST create, POST `{id}/mark-paid`, `{id}/void` |
| `payments` | GET list/show, POST create (links to invoice or standalone) |
| `contacts` | GET list/show, POST create, PATCH update |
| `documents` | GET list/metadata, GET `{id}/download`, POST upload (multipart), DELETE |
| `budgets` | GET list + vs-actual, POST create, DELETE |
| `bank-feed-links` | GET list (read-only) |

All responses SHALL use the existing RFC 7807 error envelope for 4xx/5xx.

#### Scenario: Create invoice via API then pay it in UI

- **WHEN** a client POSTs a valid 2-line invoice and then the owner
  marks it paid in the web UI
- **THEN** `GET /api/v1/ledgers/{lid}/invoices/{id}` reflects status
  `paid` and the AR aging report decreases accordingly.

#### Scenario: Viewer cannot write

- **WHEN** a viewer-scoped token POSTs to `contacts`
- **THEN** the response is 403 with an RFC 7807 body.

### Requirement: Durable Idempotency

`Idempotency-Key` on unsafe methods SHALL be stored in Postgres
`(key, token_id, request_fingerprint, response_status, response_body)`
with a 24-hour replay window. A repeated key with a different
request fingerprint SHALL return 422; a repeat within the window
SHALL replay the stored response without re-executing.

#### Scenario: Restart-safe retry

- **WHEN** a client retries a timed-out transaction creation after the
  server has restarted
- **THEN** the original response is replayed and no duplicate
  transaction exists.

#### Scenario: Key reuse with different payload

- **WHEN** the same key is sent with a different body
- **THEN** the API returns 422 and executes nothing.

### Requirement: Cursor Pagination

List endpoints SHALL accept `?limit=` (1–200, default 50) and an
opaque `?cursor=`; responses SHALL include `Link: <…>; rel="next"`
when more rows exist. Cursors MUST be tamper-evident (signed) so
client-forged cursors fail closed with 400.

#### Scenario: Stable walk

- **WHEN** a client follows `rel="next"` from a 120-row transaction list
- **THEN** pages partition the rows exactly once each.

### Requirement: Per-Token Rate Limiting

Each API token SHALL be limited to a configurable request rate
(default 120/min) using a sliding window; exceeding it returns 429
with `Retry-After`. The limit MUST apply per token, not per IP.

#### Scenario: Bursty client throttled

- **WHEN** one token issues 150 requests within a minute from two IPs
- **THEN** requests beyond the limit receive 429 while other tokens are unaffected.
