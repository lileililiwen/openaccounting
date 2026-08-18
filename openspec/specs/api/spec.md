# api Specification

## Purpose
TBD - created by archiving change a1-rest-api. Update Purpose after archive.
## Requirements
### Requirement: API Token Auth

MUST authenticate API requests with a bearer token issued from `/account/api-tokens`; the token prefix is `oa_live_`; the token is shown once at creation and stored as Argon2id-hashed at rest.

#### Scenario: Valid bearer

- **WHEN** the client sends `Authorization: Bearer oa_live_…`
- **THEN** the request is processed.

#### Scenario: Missing/invalid bearer

- **WHEN** no header or wrong token
- **THEN** 401.

#### Scenario: Revoked token

- **WHEN** a token that was revoked
- **THEN** 401.

#### Scenario: Token rotation

- **WHEN** the user revokes token A and creates token B
- **THEN** A returns 401; B works.

### Requirement: Resource Coverage

MUST expose CRUD for ledgers, accounts, and transactions; MUST expose read-only endpoints for the four standard reports.

#### Scenario: Create transaction via API

- **WHEN** the client POSTs a balanced transaction
- **THEN** 201 with the new id.

#### Scenario: List transactions

- **WHEN** the client GETs `/api/v1/ledgers/{id}/transactions`
- **THEN** 200 with a paginated JSON array.

### Requirement: Error Format

MUST return RFC 7807 problem-details JSON for every 4xx/5xx response.

#### Scenario: Validation error

- **WHEN** the client posts unbalanced lines
- **THEN** 400 with `{ "type": "…", "title": "Bad Request", "status": 400, "detail": "Postings do not balance", … }`.

### Requirement: Idempotency

MUST accept an `Idempotency-Key` header on `POST` endpoints; replaying a key returns the original 201 response without re-applying.

#### Scenario: Replay

- **WHEN** the client POSTs the same transaction twice with the same key
- **THEN** the second call returns the same 201 without writing a new row.

### Requirement: Rate Limiting

MUST rate-limit the API to 60 req/min per token (configurable via `API_RATE_PER_MINUTE`).

#### Scenario: Burst

- **WHEN** the client fires 100 requests in 60 s
- **THEN** the first 60 succeed; the rest return 429.

### Requirement: Versioning

MUST live under `/api/v1/...`; breaking changes SHALL require a new version prefix.

#### Scenario: Old prefix deprecated

- **WHEN** the client uses `/api/v1/…`
- **THEN** still supported; deprecation header `Sunset` set when v2 ships.

