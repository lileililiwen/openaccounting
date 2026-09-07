# outgoing-webhooks Specification

## Purpose
TBD - created by archiving change automation-platform. Update Purpose after archive.
## Requirements
### Requirement: Subscriptions

Ledger owners SHALL manage webhook subscriptions
(`POST /ledgers/{id}/webhooks/subscriptions`) with fields: target URL
(HTTPS only), secret (server-generated, shown once), enabled flag, and
an event-type filter list. Events SHALL include at minimum:
`transaction.posted`, `transaction.voided`, `invoice.created`,
`invoice.paid`, `invoice.overdue`, `budget.threshold_crossed`.

#### Scenario: Subscribe and filter

- **WHEN** an owner subscribes with events `[invoice.paid]`
- **THEN** posting a transaction enqueues nothing, while marking an
  invoice paid enqueues one delivery.

### Requirement: Signed Delivery

Deliveries SHALL POST a JSON envelope `{id, type, occurred_at,
ledger_id, data}` with headers `X-OA-Signature:
sha256=<HMAC-SHA256(secret, body)>`, `X-OA-Event`,
`X-OA-Delivery`. The secret SHALL never appear in any response or log.

#### Scenario: Receiver can verify authenticity

- **WHEN** a delivery arrives at the subscriber
- **THEN** HMAC over the raw body with the subscription secret verifies.

### Requirement: Delivery Log, Retries, Replay

Every attempt SHALL be recorded in `webhook_deliveries`
`(subscription_id, event_id, attempt, status_code, duration_ms,
created_at)`. Non-2xx responses retry on the scheduler's backoff up to
5 attempts then mark `failed`. Owners SHALL be able to replay any
failed delivery from the subscription page.

#### Scenario: Timeout then success

- **WHEN** the receiver returns 500 twice then 200
- **THEN** three attempt rows exist and the delivery ends `delivered`.

#### Scenario: Secret not leakable

- **WHEN** the owner re-opens the subscription edit page
- **THEN** the secret is masked and only rotatable, never displayed.

