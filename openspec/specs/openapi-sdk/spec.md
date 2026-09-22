# openapi-sdk Specification

## Purpose

The OpenAPI SDK surfaces the binary's HTTP contract to API clients and
gives automation rules a typed event surface. A versioned OpenAPI 3.1
document at `/api/openapi.yaml` covers every `/api/v1` route, and a CI
route-inventory test fails when a router adds a path without a
corresponding document entry. POST endpoints accept an optional
`Idempotency-Key` header; replays within 24 hours return the original
response without re-executing, scoped per token plus ledger. Ledger
owners can build automation rules mapping an allowlisted trigger plus
conditions to allowlisted actions, with matching events enqueued on the
existing scheduler queue with retry and backoff. A checked-in event
catalog lists every emitted event with payload schema, signing method,
and at-least-once versus at-most-once guarantee; emitting an
undocumented event fails tests. Out of scope: provider-specific webhooks
beyond the allowlisted actions and a managed API gateway.
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

