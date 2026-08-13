# bank-reconciliation Specification

## Purpose
Reconcile bank statements with ledger records to ensure cash balances are accurate and identify discrepancies.

## Requirements

### Requirement: Bank Statement Import

The system MUST support importing bank statements in OFX, QIF,
or CSV format. The import MUST:

1. Parse the statement and extract transaction lines (date,
   description, amount, optional check number).
2. Store the imported statement lines in a `bank_statement_lines`
   table.
3. Present a reconciliation UI for matching statement lines to
   ledger transactions.

#### Scenario: Bank statement is imported

- **WHEN** a user uploads an OFX bank statement
- **THEN** the system parses the transactions and stores them
  in `bank_statement_lines` with `status='unmatched'`.

### Requirement: Bank Statement Lines Table

The system MUST maintain a `bank_statement_lines` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `account_id` UUID NOT NULL (the bank account)
- `statement_date` DATE NOT NULL
- `description` TEXT
- `amount` NUMERIC(20,4) NOT NULL
- `check_number` TEXT (nullable)
- `status` TEXT CHECK (`status` IN ('unmatched', 'matched', 'excluded'))
- `matched_transaction_id` UUID (nullable, references transactions)
- `created_at` TIMESTAMPTZ

#### Scenario: Statement line is stored

- **WHEN** an OFX import parses a line: 2026-02-01, "CHECK #1234",
  -$500.00
- **THEN** a `bank_statement_lines` row is created with
  `check_number='1234'`, `amount=-500.00`, `status='unmatched'`.

### Requirement: Reconciliation UI

The system MUST provide a reconciliation UI that:

1. Lists unmatched statement lines on the left.
2. Lists ledger transactions in the bank account on the right.
3. Allows the user to match statement lines to ledger transactions
   (many-to-one or one-to-one).
4. Shows a running "difference" between statement balance and
   ledger balance.
5. Allows excluding statement lines that don't correspond to
   ledger entries (e.g., bank fees not yet recorded).

#### Scenario: User matches statement line to transaction

- **WHEN** a user selects a $500 statement line and a $500 ledger
  transaction (CHECK #1234)
- **THEN** the statement line status changes to 'matched' and
  the transaction is marked as reconciled.

### Requirement: Reconciliation Completion

When the difference between statement balance and ledger balance
is zero (or within a tolerance), the user can "complete" the
reconciliation. Completed reconciliations MUST be logged in
the audit trail.

#### Scenario: Reconciliation is completed

- **WHEN** a user completes a reconciliation with difference = $0
- **THEN** an audit entry is created with `action='reconcile'`
  and all matched statement lines have `status='matched'`.

### Requirement: Reconciliation History

The system MUST maintain a `reconciliations` table:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `account_id` UUID NOT NULL
- `statement_date` DATE NOT NULL
- `statement_balance` NUMERIC(20,4)
- `ledger_balance` NUMERIC(20,4)
- `difference` NUMERIC(20,4)
- `completed_by` UUID
- `completed_at` TIMESTAMPTZ

The reconciliation history page MUST list past reconciliations
with date, balances, and difference.

#### Scenario: User views reconciliation history

- **WHEN** a user navigates to the reconciliation history for
  a bank account
- **THEN** they see a list of past reconciliations with date,
  statement balance, ledger balance, and difference.
