# payment-tracking Specification

## Purpose
Track payments against invoices and bills to know what is outstanding. Enables cash flow forecasting and collection/d payment management.

## Requirements

### Requirement: Payment Entity

The system MUST maintain a `payments` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `contact_id` UUID (nullable, references contacts)
- `invoice_id` UUID (nullable, references invoices)
- `amount` NUMERIC(20,4) NOT NULL
- `payment_date` DATE NOT NULL
- `payment_method` TEXT CHECK (`payment_method` IN ('cash', 'check', 'bank_transfer', 'credit_card', 'other'))
- `reference` TEXT (check number, transaction ID, etc.)
- `kind` TEXT NOT NULL CHECK (`kind` IN ('received', 'made'))
- `created_at` TIMESTAMPTZ

Each payment MUST be linked to a transaction in the ledger.

#### Scenario: Customer payment is recorded

- **WHEN** a user records a $500 payment received from a customer
  via bank transfer on 2026-02-10
- **THEN** a payment record is created with `kind='received'`,
  `payment_method='bank_transfer'`, and linked to a transaction
  (debit Bank, credit AR).

### Requirement: Payment Application to Invoice

The system MUST support applying a payment to one or more
invoices. When a payment is applied:

1. The payment's `invoice_id` is set (or a junction record is
   created for partial/multi-invoice payments).
2. The invoice's `amount_paid` is incremented by the applied
   amount.
3. If `amount_paid >= total`, the invoice status changes to 'paid'.

#### Scenario: Payment is applied to multiple invoices

- **WHEN** a $1,000 payment is received and applied as $600 to
  Invoice A and $400 to Invoice B
- **THEN** both invoices have their `amount_paid` updated
  correctly, and the payment is linked to both.

### Requirement: Unapplied Payments

Payments that are not yet applied to an invoice MUST be tracked
as "unapplied" or "on-account". The system MUST provide a list
of unapplied payments so users can apply them later.

#### Scenario: Unapplied payment appears in list

- **WHEN** a $300 payment is received but not applied to any
  invoice
- **THEN** it appears in the "Unapplied Payments" list with
  contact name and date.

### Requirement: Payment Register

The system MUST provide a payment register view that lists all
payments with:

- Date
- Contact name
- Amount
- Payment method
- Reference
- Invoice number (if applied)
- Status (applied/unapplied)

The register MUST support filtering by date range, contact,
payment method, and status.

#### Scenario: User views payment register

- **WHEN** a user navigates to the payment register
- **THEN** they see a paginated list of all payments with the
  above columns and filter controls.
