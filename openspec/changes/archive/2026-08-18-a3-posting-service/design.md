# ## Context

Distributed write paths are an audit and correctness risk. A single
service with a single signature is easier to review and test.

## Goals / Non-Goals

**Goals:**
- One canonical write API.

**Non-Goals:**
- New write semantics (e.g. two-phase commit across ledgers).
- Performance optimization (the service is not a hot path; reconciliation
  imports can later bypass via `PostingService::create_many`).

## Decisions

- The service holds a `&mut PgConnection` (acquired from a `pool.begin()`).
- It is a regular struct, not a trait, to avoid over-abstraction.
- Each handler now reads a form, builds a `NewTransaction`, calls the
  service, handles errors.

## Risks / Trade-offs

- Refactor touches many handlers. Mitigate with a single PR per handler
  with a clear before/after.
- New helper signatures are not yet a public API; treat as internal for
  now.
