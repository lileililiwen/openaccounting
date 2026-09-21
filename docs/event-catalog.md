# Event Catalog

This document lists every event emitted by OpenAccounting, its payload schema, delivery guarantees, and signing method. Events are emitted via the outgoing webhook system and the automation rule engine.

## Delivery Guarantees

All events are delivered **at-least-once**. Webhook deliveries retry with exponential backoff (30 s, 2 m, 10 m, 1 h, 6 h) up to 5 attempts, then the delivery is marked dead. Consumers MUST be idempotent — re-delivery of the same `id` field is safe.

## Outgoing Webhook Signing

Every outgoing webhook delivery is signed with HMAC-SHA256 using the subscription's `secret` (a `whsec_…` value shown once on creation). The signature is sent in the `X-OA-Signature` header as `sha256=<hex>`. Consumers verify by computing `HMAC-SHA256(secret, raw_body)` and comparing with constant-time comparison.

Incoming event intake (`POST /api/events/{ledger_id}`) uses the same HMAC scheme with the ledger's `incoming_events_secret` in the `X-OA-Event-Signature` header.

## Event Types

### `transaction.posted`

Emitted when a non-draft transaction is posted.

| Field | Type | Description |
|-------|------|-------------|
| `transaction_id` | UUID | The posted transaction |
| `number` | string | Transaction number |

**Sources:** `PostingService::create`, `template_run::scan_and_run`

---

### `transaction.voided`

Declared but not currently emitted. Reserved for future use.

---

### `invoice.created`

Emitted when an invoice is created.

| Field | Type | Description |
|-------|------|-------------|
| `invoice_id` | UUID | The new invoice |

**Sources:** `api::invoices::create`, `recurring_invoices::scan_and_run`, `handlers::invoices::create`, `handlers::estimates::convert_to_invoice`

---

### `invoice.paid`

Emitted when an invoice is marked as paid (via API or web handler).

| Field | Type | Description |
|-------|------|-------------|
| `invoice_id` | UUID | The paid invoice |

**Sources:** `api::invoices::mark_paid`, `handlers::invoices::decide`

---

### `invoice.overdue`

Emitted during the nightly invoice reminder scan for each overdue invoice.

| Field | Type | Description |
|-------|------|-------------|
| `invoice_id` | UUID | The overdue invoice |
| `offset_day` | integer | Day offset in the reminder schedule |
| `days_past_due` | integer | Days past due date |

**Sources:** `invoice_reminders::scan`

---

### `budget.threshold_crossed`

Emitted when an expense budget crosses its alert threshold.

| Field | Type | Description |
|-------|------|-------------|
| `budget_id` | UUID | The budget |
| `account_name` | string | Budget account name |
| `threshold_pct` | number | Threshold percentage (0–1) |
| `amount` | number | Budget amount |

**Sources:** `handlers::budgets::check_alerts`

---

## Inbound Event Types

These types are accepted by the incoming event intake (`POST /api/events/{ledger_id}`) and trigger automation rule evaluation:

| Type | Description |
|------|-------------|
| `external.sync` | External data-sync trigger |
| `external.document.received` | External document received |
| `external.payment.received` | External payment notification |

Inbound events must be signed with the ledger's `incoming_events_secret` and are subject to the same at-least-once delivery guarantees as internal events when routed through automation rules.
