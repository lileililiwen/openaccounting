# expense-reimbursement Specification (delta)

## ADDED Requirements

### Requirement: Claim Entity

A claim SHALL be a header row in `reimbursement_claims` owned by one
ledger, with `id` (uuid), `ledger_id`, `author_id` (the user who
created it), `title`, `description`, `currency` (3-letter, must
equal `ledgers.base_currency`), `status` (enum: `draft |
submitted | approved | rejected | paid`), `approver_id`
(nullable), `paid_at` (nullable timestamp), `created_at`,
`updated_at`. A claim MUST belong to exactly one ledger; the
owning ledger's `base_currency` is the only currency allowed
on the claim.

#### Scenario: Author creates a draft claim

- **WHEN** an authenticated user posts
  `POST /ledgers/{id}/reimbursements` with `title`,
  `employee_name`, `currency`
- **THEN** a new claim is created with `status='draft'`,
  `short_id=<8 hex chars>`, and the response is 303 to the
  claim's `show` page.

#### Scenario: Currency mismatch is rejected

- **WHEN** the user posts a claim with a `currency` that does
  not match the ledger's `base_currency`
- **THEN** the response is 400 with the body
  `currency mismatch: claim 'EUR' vs ledger base 'USD'`.

### Requirement: Line Items

Each claim SHALL contain 1..N line items in
`reimbursement_lines` with `txn_date`, `description`, `amount`
(decimal, > 0), `currency` (must equal claim currency),
`tax_amount` (decimal, ≥ 0), `advance_amount` (decimal, ≥ 0),
and `gl_account_id` (must point at an `EXPENSE` account in
the same ledger).

#### Scenario: Author adds a line item

- **WHEN** the user posts a line to
  `POST /ledgers/{id}/reimbursements/{claim}/lines` with
  `amount` and a `gl_account_id` that is an `EXPENSE`
- **THEN** the line appears on the claim's `show` page.

#### Scenario: Non-expense gl account is rejected

- **WHEN** the user posts a line with a `gl_account_id` whose
  subtype is not `OPERATING_EXPENSE` / `COST_OF_GOODS_SOLD` /
  `NON_OPERATING_EXPENSE` / `TAX_EXPENSE`
- **THEN** the response is 400 with body
  `gl account subtype '<subtype>' is not an EXPENSE`.

### Requirement: State Machine

The state machine SHALL be:

```
draft       --submit-->  submitted
submitted   --approve--> approved
submitted   --reject-->  rejected
approved    --pay-->     paid
```

`draft → submitted` requires at least one line item and a
non-empty `title`. `submitted → approved` records
`approver_id` (must differ from the author), writes an audit
event, and creates the GL transaction. `submitted → rejected`
records the rejection reason. `approved → paid` posts the
payout.

#### Scenario: Submit an empty claim is rejected

- **WHEN** the user posts `submit` on a claim with zero lines
- **THEN** the response is 400 with body
  `cannot submit an empty claim`.

#### Scenario: Reject without reason is rejected

- **WHEN** the user posts `reject` without a `reason` field
- **THEN** the response is 400 or 422 (axum's form parser
  raises 422 for missing required fields) with body
  mentioning `reason`.

#### Scenario: A second approve is rejected with 409

- **WHEN** the user posts `approve` on a claim whose status
  is already `approved`
- **THEN** the response is 409 Conflict with body
  `claim is already approved`.

### Requirement: GL Posting on Approval

When a claim moves `submitted → approved`, the handler SHALL
create one `transactions` row with one DR leg per distinct
`gl_account_id` (for the net of `amount - advance_amount`
plus a separate leg for `tax_amount`) and one CR leg into
the ledger's `EMPLOYEE_PAYABLE` account for the net payable
(amount - advance_amount, summed across all lines).

#### Scenario: Approve creates a balanced transaction

- **WHEN** the user approves a claim with one line of 12.50 USD
  and no advance
- **THEN** the resulting transaction has a DR posting of 12.50
  on the line's `gl_account_id` and a CR posting of 12.50 on
  `EMPLOYEE_PAYABLE`, and `Σ debits == Σ credits`.

#### Scenario: Approve with tax posts both legs

- **WHEN** the user approves a claim with one line of
  `amount=100, tax_amount=20`
- **THEN** the resulting transaction has 3 postings (DR
  expense 100, DR expense 20 tax, CR Employee Payable 120)
  and remains in balance.

### Requirement: GL Clearing on Payment

When a claim moves `approved → paid`, the handler SHALL
create a payout transaction with one DR leg on
`EMPLOYEE_PAYABLE` (for the net payable carried over from the
approval transaction) and one CR leg on the
user-selected `payout_account_id`. The `payout_account_id`
MUST be of type `ASSET` and subtype in (`CURRENT_ASSET`,
`OTHER_ASSET`, or matching the cash/bank name heuristic).

#### Scenario: Pay clears the payable

- **WHEN** the user pays an approved claim whose
  Employee Payable balance is 120.00 against a Bank account
- **THEN** the resulting transaction has DR Payable 120.00
  and CR Bank 120.00, and the Employee Payable balance
  returns to 0.

#### Scenario: Pay out of a non-cash account is rejected

- **WHEN** the user posts `pay` with a `payout_account_id`
  whose type or subtype does not match the cash/bank rule
- **THEN** the response is 400 with body
  `payout account must be cash or bank (...)`.

### Requirement: Cash Advance Netting

A line may carry a non-zero `advance_amount` representing
cash the company already paid out. When the claim is
approved, the handler SHALL post a CR leg on the ledger's
`EMPLOYEE_ADVANCE` account for the sum of `advance_amount`
across all lines, reducing the advance asset. The CR
Employee Payable SHALL be reduced by the same amount so the
net payable reflects only what is still owed to the
employee.

#### Scenario: Approve with an advance nets the payable

- **WHEN** the user approves a claim with one line of
  `amount=200, advance_amount=50`
- **THEN** the resulting transaction has DR expense 150,
  CR Employee Advance 50, CR Employee Payable 150, and the
  net payable is 150 (not 200).

### Requirement: Ownership and Visibility

All handlers MUST call `ledgers::ensure_owner` so a user
who is not the owner of the ledger cannot view or mutate
any of its reimbursement claims. The `viewer` role
(read-only `ledger_members`) MUST NOT be able to approve
or pay a claim (the v1 implementation enforces ownership
strictly; the `viewer` role is enforced at the membership
layer).

#### Scenario: Non-owner is rejected

- **WHEN** a user who is not the owner of the ledger posts
  to any of the reimbursement routes
- **THEN** the response is 404 (the standard
  `ensure_owner` response).

### Requirement: List View and Filters

`GET /ledgers/{id}/reimbursements` SHALL list claims in the
ledger, newest first. The list page MUST show: claim short
id, title, author (employee name), status, currency, and
the total (sum of line `amount`s).

#### Scenario: List shows every claim

- **WHEN** the user visits the list page with three claims
  in the ledger
- **THEN** the page renders all three rows with the
  documented columns.

### Requirement: Receipt Attachment Per Line

A line MAY carry a receipt. The system SHALL support attaching
a `documents` row to a line. If the user submits a claim
without receipts, the preview MUST surface a soft warning
"no receipt attached" but MUST NOT block the submit.

#### Scenario: Approve with no receipt still proceeds

- **WHEN** the user submits a claim whose lines have no
  receipts
- **THEN** the submit succeeds and the audit event
  `reimbursement.submit` records `receipts: 0`.

### Requirement: Audit Trail

Every state transition (submit, approve, reject, pay) MUST
write a `reimbursement_events` row AND a `audit_entries` row
with the actor's id and a structured payload.

#### Scenario: Approve writes both audit tables

- **WHEN** the user approves a claim
- **THEN** `reimbursement_events` has a row with
  `event_type='approve'`, and `audit_entries` has a row with
  `action='reimbursement.approve'`.
