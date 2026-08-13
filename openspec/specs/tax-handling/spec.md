# tax-handling Specification

## Purpose
Track sales tax collected and paid, generate tax reports, and simplify tax filing for small businesses.

## Requirements

### Requirement: Tax Rate Entity

The system MUST maintain a `tax_rates` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `name` TEXT NOT NULL (e.g., "State Sales Tax", "City Tax")
- `rate` NUMERIC(6,4) NOT NULL (e.g., 0.0825 for 8.25%)
- `kind` TEXT NOT NULL CHECK (`kind` IN ('sales_tax', 'purchase_tax'))
- `is_active` BOOLEAN NOT NULL DEFAULT TRUE
- `created_at` TIMESTAMPTZ

#### Scenario: Tax rate is created

- **WHEN** a user creates a sales tax rate "CA Sales Tax" at 8.25%
- **THEN** a tax rate record is stored with `rate=0.0825` and
  `kind='sales_tax'`.

### Requirement: Tax Rates on Postings

The transaction posting form MUST allow selecting a tax rate.
When a tax rate is selected on a posting, the system MUST
calculate the tax amount and create additional postings:

- Original posting: the pre-tax amount
- Tax posting: the calculated tax amount (debit Tax Receivable
  or credit Tax Payable, depending on direction)

#### Scenario: Tax is calculated on a sale

- **WHEN** a user creates a $1,000 sale posting with 8.25% sales
  tax
- **THEN** the system creates:
  - Debit Accounts Receivable $1,082.50
  - Credit Sales Revenue $1,000.00
  - Credit Sales Tax Payable $82.50

### Requirement: Tax Summary Report

The system MUST provide a tax summary report that shows:

- Total sales tax collected by tax rate for a date range
- Total purchase tax paid by tax rate for a date range
- Net tax liability (collected - paid)
- Transaction count per tax rate

The report MUST support filtering by date range and tax rate.

#### Scenario: User views tax summary for the quarter

- **WHEN** a user selects Q1 2026 (Jan-Mar) and views the tax
  summary
- **THEN** the report shows total sales tax collected, total
  purchase tax paid, and net liability for each tax rate.

### Requirement: Tax Filing Export

The system MUST support exporting tax data as CSV for use in
tax filing software. The export MUST include:

- Transaction date
- Invoice/bill number
- Customer/vendor name
- Taxable amount
- Tax rate
- Tax amount

#### Scenario: Tax data is exported

- **WHEN** a user exports tax data for Q1 2026
- **THEN** a CSV file is downloaded with all taxable transactions
  and their tax details.

### Requirement: Tax Liability Account

Each tax rate MUST be linked to a liability account (for sales
tax) or receivable account (for purchase tax). These accounts
are used when creating tax postings.

#### Scenario: Tax liability account is linked

- **WHEN** a sales tax rate is created with a linked liability
  account "Sales Tax Payable"
- **THEN** all sales tax postings are credited to that account.
