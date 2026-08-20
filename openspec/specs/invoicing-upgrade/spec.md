# invoicing-upgrade Specification

## Purpose
TBD - created by archiving change 2026-08-20-a18-invoicing-upgrade. Update Purpose after archive.
## Requirements
### Requirement: Invoice line items

MUST allow creating an invoice from one or more line items (description, quantity, unit price, amount), with the invoice total computed as the sum of line amounts and stored in `invoice_lines`.

#### Scenario: Multi-line invoice

- **WHEN** a writer creates an invoice with two lines (10 × 150.00 and 1 × 100.00)
- **THEN** the invoice total is 1600.00 and both lines are stored against it.

#### Scenario: No valid lines

- **WHEN** a writer submits an invoice with no lines or only zero amounts
- **THEN** the create is rejected.

### Requirement: Invoice detail page

MUST provide a per-invoice page showing the contact, dates, kind, line items, totals, amount paid, outstanding balance, and status, with a print button and actions to mark the invoice paid or void it.

#### Scenario: View an invoice

- **WHEN** a writer opens an invoice detail page
- **THEN** it shows the line items, total, paid, outstanding, and status.

#### Scenario: Mark paid / void

- **WHEN** a writer marks an invoice paid or voids it
- **THEN** the status updates and the action is audit-logged.

### Requirement: Overdue flag

MUST flag an open invoice whose due date is in the past and whose balance is unpaid as overdue, on both the list and the detail page.

#### Scenario: Past due

- **WHEN** an open invoice has `due_date < today` and `amount_paid < total`
- **THEN** it is shown as overdue.

#### Scenario: Not yet due

- **WHEN** an invoice is due today or later
- **THEN** it is not shown as overdue.

