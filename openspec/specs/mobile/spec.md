# mobile Specification

## Purpose
TBD - created by archiving change 2026-08-14-mobile-shells. Update Purpose after archive.
## Requirements
### Requirement: Device Registration

`POST /devices/register` MUST accept `{ token: string, platform:
"ios" | "android" }` and store it in `device_tokens`
(`id, user_id, token, platform, last_seen_at`). The handler
SHALL require authentication.

#### Scenario: User registers their device

- **WHEN** an authenticated user POSTs
  `{"token":"abc","platform":"ios"}`
- **THEN** a row is added with `user_id=<user.id>` and the
  response is `201 Created`.

### Requirement: Push on Approval Required

When a reimbursement claim's required-approval set changes
(due to a new policy or new approver), the server SHALL push
a notification to every approver's registered devices with
title `"Reimbursement awaiting your approval"`, body
`"<title> (#<short id>)"`, and data `{ url:
"/ledgers/{id}/reimbursements/{claim_id}" }`.

#### Scenario: Approver receives notification

- **WHEN** a claim transitions to `submitted` and Alice has
  `role=Admin` and one registered device
- **THEN** Alice's device receives a push within 30 seconds
  with the claim title and short id in the body.

### Requirement: Push on Sync Done

When a bank-feed sync completes and adds N new transactions,
the server SHALL push a notification to the ledger owner with
title `"Bank feed sync"`, body `"<N> new transactions on
<link_nickname>"`.

#### Scenario: User receives sync notification

- **WHEN** a Plaid link syncs 5 new transactions
- **THEN** the ledger owner receives a push within 60 seconds.

### Requirement: Push Provider Pluggability

The notifications module SHALL expose a `Notifier` trait with two
implementations in v1: `ApnsNotifier` (iOS) and
`FcmNotifier` (Android). The implementation MUST be selected by
`PUSH_PROVIDER` env var (`apns | fcm | none`); `none` SHALL disable
pushes without breaking the routes.

#### Scenario: Push disabled in dev

- **WHEN** `PUSH_PROVIDER=none` is set
- **THEN** the `/devices/register` route still works (stores
  the token) but the dispatcher no-ops; no external calls are
  made.

