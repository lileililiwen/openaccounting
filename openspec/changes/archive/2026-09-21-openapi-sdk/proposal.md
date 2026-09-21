# Proposal: Complete API surface with OpenAPI and automation

## Why

`/api/v1/*` covers ledgers, transactions, accounts, contacts, invoices, payments, budgets, documents, reports, bank feeds (`src/api/`, `api-v2-coverage`), but there is no OpenAPI document, no SDK story, and idempotency (`0052_*`) is undocumented. Webhooks are outgoing-only and the scheduler/automation jobs (`src/jobs/`, `src/workers/scheduler.rs`) have no user-visible rule builder or event catalog. Integrators cannot build reliably on this.

## What Changes

- Published OpenAPI 3.1 document served from the binary plus per-ledger coverage completion (list/create/update everywhere current gaps exist).
- Documented idempotency-key semantics and webhook signature verification guide with replay examples.
- Incoming webhook/event intake for automation triggers plus a user-facing automation rule builder (event → condition → action).
- Public event catalog document naming every event, payload schema, and delivery guarantee.

## Capabilities

### New Capabilities
- `openapi-sdk`: OpenAPI document, client generation guide, idempotency docs, incoming events, rule builder, event catalog.

### Modified Capabilities
- `api`: fills per-resource CRUD gaps without changing auth or pagination behavior.
- `outgoing-webhooks`: documents signing, rotation, and replay alongside new incoming intake.
- `scheduler-worker`: automation rules execute on the existing job queue without changing queue semantics.

## Impact

Affected: `src/api/*`, OpenAPI YAML served under `/api/openapi.yaml`, `src/jobs/*`, automation templates, `docs/api-reference.md`. Unaffected: posting math, report math, auth flows, mobile shell.
