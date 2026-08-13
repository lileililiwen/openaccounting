# ar-ap-aging Specification

## Purpose
Track amounts owed to the business (accounts receivable) and amounts the business owes (accounts payable), with aging breakdowns for cash flow management and collection/d payment prioritization.

## Requirements

### Requirement: Contacts Entity

The system MUST maintain a `contacts` table for customers and
vendors:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `name` TEXT NOT NULL
- `email` TEXT
- `phone` TEXT
- `kind` TEXT NOT NULL CHECK (`kind` IN ('customer', 'vendor', 'both'))
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

Contacts MUST be linked to transactions via a `contact_id` field
on the transactions table (nullable, for backward compatibility).

#### Scenario: Contact is created

- **WHEN** a user creates a contact with `kind='customer'`
- **THEN** the contact is stored and available for selection
  when creating transactions.

### Requirement: Invoice Entity

The system MUST maintain an `invoices` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `contact_id` UUID NOT NULL (references contacts)
- `kind` TEXT NOT NULL CHECK (`kind` IN ('receivable', 'payable'))
- `invoice_number` TEXT
- `invoice_date` DATE NOT NULL
- `due_date` DATE NOT NULL
- `total` NUMERIC(20,4) NOT NULL
- `amount_paid` NUMERIC(20,4) NOT NULL DEFAULT 0
- `status` TEXT NOT NULL CHECK (`status` IN ('open', 'paid', 'overdue', 'void'))
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

An invoice MUST be linked to one or more transactions via an
`invoice_id` field on the postings or a junction table.

#### Scenario: Receivable invoice is created

- **WHEN** a user creates an invoice for a customer with total
  $1,000 and due date 2026-02-15
- **THEN** an invoice record is created with `kind='receivable'`,
  `status='open'`, and `amount_paid=0`.

### Requirement: Payment Application

The system MUST support recording a payment against an invoice.
When a payment is recorded:

1. A transaction is created (e.g., debit Bank, credit AR).
2. The invoice's `amount_paid` is incremented.
3. If `amount_paid >= total`, the invoice status changes to 'paid'.

#### Scenario: Partial payment is recorded

- **WHEN** a $1,000 invoice receives a $400 payment
- **THEN** the invoice shows `amount_paid=400`, `status='open'`,
  and a $400 transaction is created.

#### Scenario: Full payment closes the invoice

- **WHEN** a $1,000 invoice receives a final $600 payment
  (after a prior $400 payment)
- **THEN** the invoice shows `amount_paid=1000`, `status='paid'`.

### Requirement: AR Aging Report

The system MUST provide an Accounts Receivable aging report that
shows, for each customer with open invoices:

- Customer name
- Current (not yet due)
- 1-30 days past due
- 31-60 days past due
- 61-90 days past due
- 90+ days past due
- Total outstanding

The aging buckets MUST be computed relative to the report date.
Invoices with `status='void'` MUST be excluded.

#### Scenario: AR aging shows past-due amounts

- **WHEN** a customer has invoices due on 2026-01-15 (90+ days),
  2026-03-01 (31-60 days), and 2026-04-01 (1-30 days)
- **THEN** the AR aging report shows amounts in the correct
  buckets for that customer.

### Requirement: AP Aging Report

The system MUST provide an Accounts Payable aging report that
shows, for each vendor with open invoices:

- Vendor name
- Current (not yet due)
- 1-30 days past due
- 31-60 days past due
- 61-90 days past due
- 90+ days past due
- Total outstanding

#### Scenario: AP aging shows amounts owed by period

- **WHEN** a vendor has invoices due on various dates
- **THEN** the AP aging report shows amounts in the correct
  aging buckets.

### Requirement: Invoice Dashboard Widget

The dashboard MUST display a summary widget showing:

- Total AR outstanding
- Total AP outstanding
- Number of overdue invoices (AR)
- Number of overdue bills (AP)

#### Scenario: Dashboard shows AR/AP summary

- **WHEN** a user views the dashboard
- **THEN** a widget shows total AR, total AP, and overdue counts.
