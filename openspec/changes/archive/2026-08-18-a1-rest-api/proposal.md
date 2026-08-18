# Add a Public REST API (v1)

## Why

OpenAccounting only has HTML form posts. There is no JSON surface, so
external scripts (OFX importers, mobile apps, accountants' tools) cannot
integrate. Firefly III ships a ~200-endpoint REST API; Akaunting ships a
full Laravel REST surface. Even a tiny v1 enables the next ten
integrations.

## What Changes

- New `/api/v1/` prefix.
- Endpoints: ledgers (list, get, create), accounts (list, get, create,
  update), transactions (list, get, create, edit, reverse), reports
  (trial-balance, balance-sheet, income-statement, cash-flow,
  general-ledger).
- Authentication: bearer token (per-user API token stored in
  `api_tokens`); revocable.
- Content-Type: `application/json`. Errors return RFC 7807 problem
  details.
- Idempotent reads via ETag headers.

## Capabilities

### New Capabilities

- `api`: Public REST API for third-party access.

## Impact

**New files:**
- `src/api/mod.rs`, `src/api/ledgers.rs`, `src/api/accounts.rs`,
  `src/api/transactions.rs`, `src/api/reports.rs`.
- `migrations/0031_add_api_tokens.sql`.
- `tests/http/api.rs`.

**Modified files:**
- `src/lib.rs` — mount `/api/v1`.
- `src/auth/` — bearer-token middleware.
- `Cargo.toml` — no new deps (serde_json already present).
