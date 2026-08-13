# data-import Specification

## Purpose
Enable businesses to import historical transactions from spreadsheets, other accounting systems, and bank statements. Reduces barriers to adoption.

## Requirements

### Requirement: CSV Transaction Import

The system MUST support importing transactions from a CSV file.
The import workflow MUST:

1. Accept a CSV file upload.
2. Display a column-mapping UI where the user maps CSV columns
   to transaction fields (date, description, account, debit,
   credit, payee, reference).
3. Preview the first 10 rows before committing.
4. Validate all rows (balanced debits/credits, valid accounts,
   valid dates).
5. Insert all valid rows as transactions, or reject all if any
   row is invalid (atomic import).

#### Scenario: CSV import with column mapping

- **WHEN** a user uploads a CSV with columns "Date", "Desc",
  "Debit", "Credit" and maps them to the correct fields
- **THEN** the system previews the first 10 rows and the user
  can confirm the import.

#### Scenario: Invalid CSV row is rejected

- **WHEN** a CSV row has a date in an invalid format
- **THEN** the entire import is rejected with an error message
  indicating the problematic row and column.

### Requirement: Duplicate Detection

The system MUST detect potential duplicate transactions during
import. A transaction is a potential duplicate if it has the
same date, description, and amount as an existing transaction
in the same ledger. The user MUST be able to override duplicate
detection and force-import.

#### Scenario: Duplicate is flagged

- **WHEN** a CSV import contains a transaction that matches an
  existing transaction by date+description+amount
- **THEN** the import preview flags the row as a potential
  duplicate and the user can choose to skip or include it.

### Requirement: OFX/QIF Bank Statement Import

The system MUST support importing bank statements in OFX
(Open Financial Exchange) and QIF (Quicken Interchange Format)
formats. The import MUST:

1. Parse the statement file.
2. Extract transaction lines (date, description, amount,
   optional check number).
3. Display a preview of parsed transactions.
4. Allow the user to map each bank line to an account and
   direction (debit/credit).
5. Create transactions from the mapped lines.

#### Scenario: OFX file is imported

- **WHEN** a user uploads an OFX bank statement
- **THEN** the system parses the transactions and displays them
  in a preview table with date, description, and amount columns.

### Requirement: Import Log

Every import operation MUST be logged in the audit trail with:

- `action='import'`
- `entity_type='transaction'`
- `new_value` containing summary: file name, row count, account
  mapping

#### Scenario: Import is audited

- **WHEN** a user successfully imports 50 transactions from CSV
- **THEN** an audit entry records the import with file name,
  row count, and target account.
