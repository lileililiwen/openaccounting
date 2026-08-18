# split-transaction-ux Specification (delta)

## ADDED Requirements

### Requirement: Add Split

MUST allow the user to add additional rows to a transaction; the rows are persisted as multiple postings.

#### Scenario: Three-leg split

- **WHEN** the user adds two extra rows to a $300 expense
- **THEN** all three rows are persisted; net is zero.

### Requirement: Auto-Balance

MUST show the running balance on the form; the difference between debits and credits MUST be displayed live; an `Auto-balance` button MUST fill the empty row with the missing amount and direction.

#### Scenario: Auto-balance

- **WHEN** the user enters debits of 100 and 50 and clicks Auto-balance on the last row
- **THEN** the last row is filled with a 50 credit.

### Requirement: Reorder Rows

MUST allow reordering of rows via drag-and-drop using HTMX swap.

#### Scenario: Reorder

- **WHEN** the user drags row 2 above row 1
- **THEN** the form submission posts the rows in the new order; the report renders correctly.

### Requirement: Remove Row

MUST allow removing any row other than the first.

#### Scenario: Remove middle

- **WHEN** the user removes the second of three rows
- **THEN** the form now has two rows; submission works.

### Requirement: Validation

MUST reject submission with fewer than two non-empty rows; the same server-side check applies (no JS dependency).

#### Scenario: Server rejects single row

- **WHEN** the user disables JS and submits one row
- **THEN** 422.
