# recurring-transactions Specification

## Purpose
Automate repetitive transaction entry for regular payments (rent, payroll, subscriptions) and receipts. Reduces manual data entry and prevents missed entries.

## Requirements

### Requirement: Transaction Template Entity

The system MUST maintain a `transaction_templates` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `description` TEXT NOT NULL
- `payee` TEXT
- `reference` TEXT
- `frequency` TEXT NOT NULL CHECK (`frequency` IN ('weekly', 'biweekly', 'monthly', 'quarterly', 'yearly'))
- `next_date` DATE NOT NULL
- `is_active` BOOLEAN NOT NULL DEFAULT TRUE
- `created_at` TIMESTAMPTZ
- `updated_at` TIMESTAMPTZ

Each template MUST have one or more posting lines stored in a
`template_postings` table:

- `id` UUID PRIMARY KEY
- `template_id` UUID NOT NULL
- `account_id` UUID NOT NULL
- `direction` TEXT NOT NULL CHECK (`direction` IN ('DEBIT', 'CREDIT'))
- `amount` NUMERIC(20,4) NOT NULL
- `memo` TEXT

#### Scenario: Monthly rent template is created

- **WHEN** a user creates a template for monthly rent of $2,000
  (debit Rent Expense, credit Bank Account)
- **THEN** a template record with two posting lines is stored,
  with `next_date` set to the first upcoming rent date.

### Requirement: Auto-Generation of Recurring Transactions

The system MUST auto-generate transactions from templates when
their `next_date` falls on or before today. The auto-generation
MUST:

1. Create a transaction with `kind='recurring'` (not 'standard').
2. Copy the template's description, payee, reference, and
   posting lines.
3. Use the template's `next_date` as the transaction date.
4. Advance `next_date` by the template's frequency.
5. Record the generation in the audit log.

#### Scenario: Monthly subscription is auto-generated

- **WHEN** today is 2026-02-01 and a template has
  `next_date=2026-02-01` with `frequency='monthly'`
- **THEN** a transaction is created with `txn_date=2026-02-01`
  and the template's `next_date` advances to 2026-03-01.

### Requirement: Template Management UI

The system MUST provide a UI to:

- List all templates for a ledger (active and inactive).
- Create a new template with posting lines.
- Edit an existing template.
- Toggle a template active/inactive.
- Manually trigger a template (generate a transaction now).

#### Scenario: User toggles template inactive

- **WHEN** a user deactivates a monthly template
- **THEN** `is_active` changes to false and no future transactions
  are auto-generated from it.

### Requirement: Recurring Transaction Indicator

Transactions generated from templates MUST be visually indicated
in the transaction list (e.g., with a recurring icon or badge).
The transaction detail page MUST show which template generated it.

#### Scenario: Generated transaction shows template link

- **WHEN** a user views a transaction that was auto-generated
  from a template
- **THEN** the detail page shows "Generated from: Monthly Rent"
  with a link to the template.
