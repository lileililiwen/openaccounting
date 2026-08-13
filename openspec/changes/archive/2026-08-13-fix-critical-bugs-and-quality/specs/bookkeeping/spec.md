# bookkeeping Specification

## Purpose
TBD - created by archiving change fix-critical-bugs-and-quality. Update Purpose after archive.

## MODIFIED Requirements

### Requirement: Transaction Form

The "new transaction" page MUST:

- Show a date input defaulted to today.
- Require a non-empty `description`.
- Allow optional `payee` and `reference` fields.
- Render N posting rows (default 2), each with:
  - an account `<select>` populated from the ledger's chart of
    accounts (excluding archived);
  - a `DEBIT` / `CREDIT` `<select>`;
  - a positive `<input type="number" step="0.01" min="0.01">`.
- Provide an "+ Add line" button that appends a new posting row via
  inlined JavaScript (no JS build).
- On submission, validate that at least two postings exist and that
  the absolute value of the sum of signed amounts is `0`.
- The form fields MUST use distinct indices (`lines[N][field]`)
  so that each posting is stored separately. The JavaScript "Add
  line" handler MUST rewrite indices when cloning posting rows.

If validation fails, the form is re-rendered with the user's
input preserved and a clear error message.

#### Scenario: User submits an unbalanced form

- **WHEN** the user submits a transaction with debit `100.00` to Cash
  and credit `99.00` to Sales Revenue
- **THEN** the response is HTTP 200 (form re-render) with the error
  banner "Postings do not balance: net is 1.00 (debits must equal
  credits)."

#### Scenario: User adds a third posting

- **WHEN** the user clicks "+ Add line" on an empty form with 2
  default rows
- **THEN** a third posting row is added with empty inputs and
  indexed `lines[2]`.

#### Scenario: Two postings are submitted correctly

- **WHEN** a user submits a transaction with two posting rows
  (e.g. debit Cash 100, credit Revenue 100)
- **THEN** the handler receives two distinct posting entries
  with indices 0 and 1, and both are stored in the database.

### Requirement: Per-Ledger Ownership

Every `ledger`, `account`, `transaction`, and `document` belongs to
exactly one user (the ledger's `owner_id`). All read / write
operations on these resources MUST verify that the authenticated
user is the owner; if not, the handler returns HTTP 404 (not 403)
to avoid leaking the existence of the ledger.

#### Scenario: User A tries to read user B's ledger

- **WHEN** user A requests `GET /ledgers/<B's ledger id>/dashboard`
- **THEN** the response is HTTP 404 (not 403, to avoid leaking
  existence).

#### Scenario: Non-owner accesses a ledger resource

- **WHEN** a user requests a resource (e.g. `GET /ledgers/{id}/accounts`)
  for a ledger they do not own
- **THEN** the response is `404 Not Found`. The response does
  not reveal whether the ledger exists or whether the user lacks
  access.
