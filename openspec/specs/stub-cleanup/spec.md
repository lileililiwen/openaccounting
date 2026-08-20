# stub-cleanup Specification

## Purpose
TBD - created by archiving change 2026-08-20-a19-stub-cleanup. Update Purpose after archive.
## Requirements
### Requirement: API token management UI

MUST make the API token page reachable: a logged-in user SHALL be able to list, issue (seeing the plaintext once), and revoke REST API tokens.

#### Scenario: Issue a token

- **WHEN** a logged-in user opens `/account/api-tokens` and creates a token
- **THEN** the token is shown once and appears in the list.

#### Scenario: Revoke a token

- **WHEN** a user revokes a token
- **THEN** it is marked revoked and no longer usable.

### Requirement: Plaid webhook signature verification

SHALL verify the `Plaid-Verification` header (HMAC-SHA256 over the raw body, `v1:` scheme) when `PLAID_WEBHOOK_SECRET` is configured, rejecting mismatches and unknown schemes.

#### Scenario: Valid signature

- **WHEN** a webhook arrives with a correct `v1:` HMAC-SHA256 signature
- **THEN** verification succeeds.

#### Scenario: Invalid signature

- **WHEN** the signature is wrong, the scheme is unknown, or the header is malformed
- **THEN** verification fails and the request is rejected.

### Requirement: Inter-ledger transfer form

MUST provide a working transfer form at `/transfers/inter-ledger` posting the existing create endpoint, listing the user's ledgers.

#### Scenario: Render the form

- **WHEN** a user opens the transfer form
- **THEN** it lists their ledgers and the transfer fields.

#### Scenario: Create a transfer

- **WHEN** a user submits a transfer between two ledgers
- **THEN** two transactions and the inter-ledger link are created.

