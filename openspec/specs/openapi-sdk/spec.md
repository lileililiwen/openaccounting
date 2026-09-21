# openapi-sdk Specification

## Purpose
TBD - created by archiving change openapi-sdk. Update Purpose after archive.
## Requirements
### Requirement: OpenAPI Contract

The binary SHALL serve a versioned OpenAPI 3.1 document at `/api/openapi.yaml` covering every `/api/v1` route. CI SHALL fail when a route lacks a corresponding document entry.

#### Scenario: New route without docs fails CI

- **WHEN** a router adds `GET /ledgers/{id}/widgets` without an OpenAPI entry
- **THEN** the route-inventory test fails naming the missing path.

### Requirement: Idempotent Writes

POST endpoints SHALL accept an optional `Idempotency-Key` header; replays within 24h SHALL return the original response without re-executing. Keys SHALL be scoped per token plus ledger.

#### Scenario: Retry does not duplicate

- **WHEN** a client posts a transaction with key K, times out, and retries with key K
- **THEN** one transaction exists and both responses are identical.

### Requirement: Incoming Events and Rule Builder

Ledger owners SHALL create automation rules mapping an allowlisted trigger plus conditions to allowlisted actions. Matching events SHALL enqueue jobs on the existing scheduler queue with retry/backoff.

#### Scenario: Overdue invoice triggers webhook

- **WHEN** a rule "invoice overdue → POST webhook W" exists and invoice I becomes overdue
- **THEN** one webhook delivery job for W is enqueued with the documented payload.

### Requirement: Event Catalog

A checked-in catalog SHALL list every event with payload schema, signing method, and at-least-once vs at-most-once guarantee. Undocumented events SHALL NOT be emitted.

#### Scenario: Unknown event blocked

- **WHEN** code attempts to emit an event missing from the catalog
- **THEN** tests fail naming the unlisted event.

