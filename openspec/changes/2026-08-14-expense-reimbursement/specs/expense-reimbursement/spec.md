# expense-reimbursement Specification

## Purpose

Define the **claim → line items → state machine → GL posting**
workflow that lets a small-business operator reimburse an
employee's out-of-pocket expenses while keeping the books
provably balanced.

## Requirements

### Requirement: Claim Entity

A claim is a header row in `reimbursement_claims` owned by one
ledger, with:

- `id` (uuid), `ledger_id`, `author_id` (the user who created it),
  `title` (e.g. "2026 Q3 client visits"), `description`,
  `currency` (3-letter, must equal `ledgers.base_currency`),
  `status` (enum: `draft | submitted | approved | rejected | paid`),
  `approver_id` (nullable user id), `paid_at` (nullable
  timestamp), `created_at`, `updated_at`.

A claim MUST belong to exactly one ledger. The owning ledger's
`base_currency` is the only currency allowed on the claim.

#### Scenario: Author creates a draft claim

- **WHEN** an authenticated owner of ledger L posts
  `POST /ledgers/L/reimbursements` with `title="Client visits"`
  and `currency="CNY"`
- **THEN** a new claim row is created with `status='draft'`,
  `author_id=<user.id>`, `currency="CNY"`, zero line items, and
  the response is `303 See Other` to
  `/ledgers/L/reimbursements/<new claim id>`.

#### Scenario: Currency mismatch is rejected

- **WHEN** the author posts a claim with `currency="USD"` on a
  ledger whose `base_currency="CNY"`
- **THEN** the handler returns `400 Bad Request` with body
  `Currency must match ledger base currency (CNY).`.

### Requirement: Line Items

Each claim contains 1..N line items in `reimbursement_lines`:

- `id`, `claim_id`, `txn_date` (date), `category` (one of a fixed
  enum: `travel | meals | lodging | supplies | software |
  other`), `payee` (free text), `description`, `amount` (decimal,
  > 0), `currency` (must equal claim currency), `tax_amount`
  (decimal, ≥ 0), `tax_recoverable` (boolean), `gl_account_id`
  (must point at an `EXPENSE` account in the same ledger),
  `project_id` (nullable), `receipt_document_id` (nullable,
  references the existing `documents` table).

The line total stored is `amount + tax_amount`. The recoverable
tax portion is posted to a separate "Input VAT" account if the
ledger has one; otherwise the entire `amount + tax_amount` is
posted to `gl_account_id`.

#### Scenario: Author adds a line item

- **WHEN** the author posts
  `POST /ledgers/L/reimbursements/<id>/lines` with `txn_date=
  2026-08-14`, `category=meals`, `amount=120.00`, `tax_amount=
  13.00`, `tax_recoverable=true`, `gl_account_id=<id of "Travel &
  Meals">`
- **THEN** a row is added and the response is `303 See Other`
  back to the claim show page, with the line visible in the
  table.

#### Scenario: GL account must be an EXPENSE

- **WHEN** the author submits a line with
  `gl_account_id=<id of "Bank Account">` (type=ASSET)
- **THEN** the handler returns `400 Bad Request` with body
  `gl_account_id must reference an EXPENSE account.`.

### Requirement: State Machine

Transitions are:

```
draft       --submit-->  submitted
submitted   --approve--> approved
submitted   --reject-->  rejected
approved    --pay-->     paid
```

`draft → submitted` requires at least one line item and a non-empty
`title`. `submitted → approved` records `approver_id` (must differ
from `author_id` if the ledger has more than one user; in v1 the
approver can be the same user — single-operator SMBs are allowed
to self-approve with an explicit `confirm_self_approve=1` flag in
the form). `submitted → rejected` requires `reject_reason` text.
`approved → paid` requires selecting a payout cash/bank account
(`payout_account_id`).

All transitions write a row to `reimbursement_events` with
`actor_id`, `from_status`, `to_status`, `reason` (nullable), and
`created_at`. The audit module is also called with the same data.

#### Scenario: Submit requires a line

- **WHEN** the author posts `/submit` on a draft claim with zero
  lines
- **THEN** the handler returns `400 Bad Request` with body
  `Cannot submit a claim with no line items.`.

#### Scenario: Rejection requires a reason

- **WHEN** anyone posts `/reject` without a `reject_reason`
  parameter
- **THEN** the handler returns `400 Bad Request` with body
  `reject_reason is required.`.

#### Scenario: Approve is idempotent

- **WHEN** an already-approved claim receives a second `/approve`
- **THEN** the response is `409 Conflict` with body
  `Claim is already approved.`.

### Requirement: GL Posting on Approval

When a claim moves `submitted → approved`, the handler SHALL
create one balanced double-entry transaction per claim, with one
posting per unique GL account appearing in the line items, plus
two postings for the payable side.

The postings are:

- **Per line, debited leg:**
  - If `tax_recoverable = true` and the ledger has an `Input VAT`
    (EXPENSE / `input_vat` subtype) account:
    - DR `gl_account_id`     amount
    - DR `Input VAT`         tax_amount
  - Else:
    - DR `gl_account_id`     amount + tax_amount
  - CR `Employee Payable`   (sum of all debit legs)

All postings share the same `transaction_id`. The transaction's
description is `"Reimbursement <claim title> (#<claim short id>)"`.
The transaction's date is the approval timestamp (UTC).

The `check_posting_balance` trigger from migration
`0001_init.sql` SHALL apply unchanged.

#### Scenario: Single-line approval posts two postings

- **WHEN** a claim with one line (120.00 / 13.00 tax /
  `tax_recoverable=true`) is approved, and the ledger has an
  `Input VAT` account
- **THEN** exactly one transaction with three postings is created
  in the ledger:
  - DR `Travel & Meals`  120.00
  - DR `Input VAT`        13.00
  - CR `Employee Payable` 133.00

#### Scenario: Approval is atomic with the state transition

- **WHEN** the approval handler runs but the GL insert fails
  (e.g. trigger violation)
- **THEN** the entire transaction is rolled back: the claim
  remains in `submitted` status, zero rows are added to
  `transactions` / `postings`, and the response is `422
  Unprocessable Entity` with the trigger message.

### Requirement: GL Clearing on Payment

When a claim moves `approved → paid`, the handler SHALL create
one balanced transaction:

- DR `Employee Payable`   (sum of approved debits)
- CR `payout_account_id`  (same sum)

The transaction's description is `"Reimbursement payout <claim
title> (#<claim short id>)"`. The transaction's date is the
payment timestamp (UTC). The `paid_at` column on the claim is
set to the same timestamp.

The `payout_account_id` MUST be of type `ASSET` and subtype in
(`cash`, `bank`); otherwise the handler returns `400 Bad Request`.

#### Scenario: Successful payout clears the payable

- **WHEN** an approved claim is paid via `payout_account_id=<id of
  "Bank Account">` (ASSET/bank)
- **THEN** one transaction with two postings is created, the
  claim's `status` becomes `paid`, `paid_at` is set to the
  payment timestamp.

### Requirement: Cash Advance Netting

A line item MAY reference an employee advance via
`applied_advance_id` (nullable, references a future
`reimbursement_advances` table or simply an `Employee Advance`
account posting). When the claim is approved, the handler
subtracts the applied-advance amount from the CR `Employee
Payable` total and posts a separate leg:

- DR `Employee Advance`  applied_amount
- CR `Employee Payable`  applied_amount

Net payable = sum(debits) − sum(applied advances).

In v1 the `applied_advance_id` field on a line MAY be null. If
non-null, it MUST point at a transaction in the same ledger
whose CR leg is `Employee Advance`. If that invariant fails, the
handler returns `400 Bad Request`.

#### Scenario: Netting an advance

- **WHEN** a single-line claim of 133.00 has
  `applied_advance_id=<txn id of a 50.00 advance>`
- **THEN** the approval transaction has four postings:
  - DR `Travel & Meals`    120.00
  - DR `Input VAT`          13.00
  - DR `Employee Advance`    50.00
  - CR `Employee Payable`  183.00

### Requirement: Receipt Attachment Per Line

Each line item MAY link to one row in `documents` via
`receipt_document_id`. The `documents` upload route already
supports per-transaction uploads; this change extends the upload
endpoint with an optional `claim_line_id` parameter so a single
upload can be attached to a claim line.

A line item without `receipt_document_id` MAY be approved; the UI
MUST surface a soft warning "no receipt attached" but MUST NOT
block approval (the policy engine is out of scope for v1).

#### Scenario: Receipt attaches to line

- **WHEN** the user uploads a PDF on the claim show page and
  selects a target line
- **THEN** the document row's `transaction_id` is `NULL` and the
  new `claim_line_id` column is set; the line's
  `receipt_document_id` is set to the document's id.

### Requirement: Ownership and Visibility

A claim is visible only to its author and to other users who have
been invited to share the ledger with `role in (admin, accountant)`
(via the existing `ledger_sharing` migration `0008`). A user with
`viewer` role can read but not transition state.

All handlers MUST call `ledgers::ensure_role` (new helper that
extends `ensure_owner`) which returns `AppError::NotFound` for
non-shared ledgers and `AppError::Forbidden` for read-only access.

#### Scenario: Viewer cannot approve

- **WHEN** a user with `role=viewer` on the ledger posts `/approve`
- **THEN** the response is `403 Forbidden` with body
  `You do not have permission to approve claims.`.

### Requirement: List View and Filters

`GET /ledgers/{id}/reimbursements` SHALL list claims in the
ledger, newest first, with these query params:

- `status` (optional, comma-separated subset of
  `draft,submitted,approved,rejected,paid`).
- `author_id` (optional).
- `from` / `to` (optional date range, applied to `txn_date` of the
  latest line).

The list page MUST show: claim short id, title, author, line count,
total amount, status badge, and a link to the show page.

#### Scenario: Filter by status

- **WHEN** the user requests
  `?status=submitted,approved`
- **THEN** only claims with those statuses are returned.
