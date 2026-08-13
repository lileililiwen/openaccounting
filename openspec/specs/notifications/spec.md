# notifications Specification

## Purpose
Alert users to important events: overdue invoices, upcoming bill due dates, budget thresholds, and system events.

## Requirements

### Requirement: Notification Entity

The system MUST maintain a `notifications` table:

- `id` UUID PRIMARY KEY
- `user_id` UUID NOT NULL
- `ledger_id` UUID (nullable, for ledger-scoped notifications)
- `kind` TEXT NOT NULL CHECK (`kind` IN ('overdue_invoice', 'upcoming_bill', 'budget_threshold', 'system'))
- `title` TEXT NOT NULL
- `message` TEXT NOT NULL
- `is_read` BOOLEAN NOT NULL DEFAULT FALSE
- `action_url` TEXT (nullable, link to relevant page)
- `created_at` TIMESTAMPTZ

#### Scenario: Overdue invoice notification is created

- **WHEN** an invoice passes its due date without being paid
- **THEN** a notification with `kind='overdue_invoice'` is
  created for the ledger owner with a message like "Invoice
  #1001 from Acme Corp is 15 days overdue ($2,500.00)".

### Requirement: Overdue Invoice Alerts

The system MUST automatically generate notifications when
invoices become overdue. The check MUST run:

1. On ledger access (lazy check).
2. Via a daily background job (if available).

When an invoice's `due_date` is in the past and `status != 'paid'`,
a notification is created (if one doesn't already exist for that
invoice).

#### Scenario: Notification for overdue invoice

- **WHEN** Invoice #1001 has `due_date=2026-01-15` and today
  is 2026-02-01
- **THEN** a notification is created: "Invoice #1001 from Acme
  Corp is 17 days overdue ($2,500.00)" with
  `action_url='/invoices/1001'`.

### Requirement: Upcoming Bill Alerts

The system MUST generate notifications for bills (AP invoices)
with due dates within the next 7 days.

#### Scenario: Notification for upcoming bill

- **WHEN** an AP invoice has `due_date=2026-02-10` and today
  is 2026-02-05
- **THEN** a notification is created: "Bill #INV-200 from
  Vendor X is due in 5 days ($1,200.00)".

### Requirement: Budget Threshold Alerts

When a budget is set for an account and the actual spending
exceeds a configurable threshold (default 80%), the system MUST
generate a notification.

#### Scenario: Budget threshold notification

- **WHEN** the Marketing account has a $5,000 monthly budget
  and spending reaches $4,200 (84%)
- **THEN** a notification is created: "Marketing account has
  reached 84% of its monthly budget ($4,200 / $5,000)".

### Requirement: Notification UI

The system MUST provide a notifications page that:

- Lists all notifications for the current user.
- Shows unread count in the navigation bar (badge).
- Allows marking notifications as read (individually or all).
- Filters by kind (overdue, upcoming, budget, system).
- Links to the relevant entity via `action_url`.

#### Scenario: User views notifications

- **WHEN** a user clicks the notifications bell icon
- **THEN** they see a dropdown or page with unread notifications
  at the top, each with a link to the relevant invoice/bill/budget.
