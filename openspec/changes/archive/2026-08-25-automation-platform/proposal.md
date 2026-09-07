# Automation Platform: Scheduler, Notification Channels, Outgoing Webhooks

## Why

Three "the system should run itself" gaps, all visible in competitor
parity research:

1. **Recurring transactions never fire on their own.** Templates exist
   (`/templates`, `process_due`) but nothing calls `process_due` — the
   only workers are bank-feed sync, login-prune, audit anchor, and
   backups. Xero/QBO/Zoho post recurring documents automatically.
2. **No email channel at all.** Notifications are push-only and the
   FCM adapter is a stub that logs and returns Ok
   (`src/notifications/fcm.rs`). Firefly III ships email, Slack,
   Discord, Pushover, ntfy. Overdue-invoice reminders (FreshBooks,
   Sage auto-chasing) are impossible without a channel.
3. **No outgoing webhooks.** The only webhook is inbound Plaid.
   Firefly III (with delivery logs + retries), Invoice Ninja, Kill Bill,
   Bigcapital all emit events; our REST API consumers must poll.

## What Changes

- A generic background job runner (DB-backed queue) executes due work:
  recurring templates, FX refresh hook, invoice reminders, webhook
  deliveries, notification fan-out.
- Notification channels: SMTP email plus generic HTTP (ntfy-compatible)
  channel, per-user preferences, wired to budget alerts, overdue
  invoices, and weekly digest.
- Outgoing webhooks: ledger-scoped subscriptions to typed events
  (`transaction.posted`, `invoice.paid`, …), HMAC-SHA256 signed,
  delivery log with retries and manual replay.

## Capabilities

### New Capabilities

- `scheduler-worker`: DB-backed job queue + workers for due templates,
  reminders, and retries.
- `notification-channels`: SMTP + generic-HTTP delivery with per-user
  preferences.
- `outgoing-webhooks`: event subscriptions, signed deliveries, logs,
  replay.

## Impact

**New files:** `src/workers/scheduler.rs`, `src/jobs/` (queue +
handlers), `src/handlers/webhooks_out.rs`, `src/notifications/email.rs`,
`migrations/00xx_jobs.sql`, `00xx_webhook_subscriptions.sql`,
`00xx_webhook_deliveries.sql`.
**Modified:** `src/workers/mod.rs` (spawn scheduler), templates handler
(delegates to queue), notifications service (channel registry).
