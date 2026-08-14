# expense-reimbursement Specification

## Purpose
TBD - created by archiving change 2026-08-14-expense-reimbursement. Update Purpose after archive.
## Requirements
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

### Requirement: Policy Engine

A ledger SHALL be able to have zero or more
`reimbursement_policies`. Each policy has:

- `id`, `ledger_id`, `name`, `kind`
  (`category_cap | receipt_required | per_diem`),
- `config_json` (per-kind configuration),
- `severity` (`hard | soft`, default `hard`),
- `is_active` (boolean, default `true`),
- `created_at`.

A policy SHALL be active when `is_active=true`. Only active
policies are evaluated.

#### Scenario: A policy row is created via SQL

- **WHEN** the operator inserts a policy row via
  `INSERT INTO reimbursement_policies (...)`
- **THEN** the row is queryable, the `kind` matches the
  CHECK constraint, and `is_active` defaults to `true`.

### Requirement: Category Cap Policy

A `category_cap` policy's `config_json` SHALL be
`{"category":"<name>","max_per_day":<decimal>,"currency":"<code>"}`.
The policy fires when the sum of that category's lines for
the same `txn_date` exceeds `max_per_day`. One violation
per (category, date) pair SHALL be returned (deduped).

#### Scenario: Cap exceeded

- **WHEN** the ledger has a 100 USD / day cap on
  `Other Expense` and a claim has a 200 USD line on
  `Other Expense` dated today
- **AND WHEN** the user posts `submit`
- **THEN** the response is 400 with body
  `policy violation: [<policy name>] category 'Other
  Expense' cap exceeded (actual 20000¢ > cap 10000¢)`.

### Requirement: Receipt Required Policy

A `receipt_required` policy's `config_json` SHALL be
`{"min_amount":<decimal>}`. The policy fires when a line
item's `amount >= min_amount` AND the line has no receipt
attached. The line is in violation.

#### Scenario: Receipt missing on a large line

- **WHEN** the ledger has a 50 USD receipt-required policy
  AND the user submits a claim with a 100 USD line that has
  no receipt
- **THEN** the response is 400 with body
  `policy violation: [<name>] receipt required for 10000¢
  (min 5000¢)`.

### Requirement: Per-Diem Policy

A `per_diem` policy's `config_json` SHALL be
`{"destination":"<string>","daily_rate":<decimal>,"currency":"<code>"}`.
The policy fires when a lodging line item's amount exceeds
`daily_rate * nights`. Nights SHALL be derived from the
difference between this line's `txn_date` and the previous
line of the same category in the same claim.

#### Scenario: Per-diem over

- **WHEN** a claim has two lodging lines dated 2026-08-13
  and 2026-08-15 (2 nights), and a Berlin per-diem of
  100 EUR / night, but the line amounts are 500 EUR each
- **THEN** the policy fires and the violation amount is
  300 EUR (200 over the 2-night allowance of 200).

### Requirement: Evaluation Timing

Policies SHALL be evaluated at three points:

1. **Line add** — soft-severity policies return warnings
   in the `add_line` response (no blocking).
2. **Claim submit** — hard-severity violations block submit
   with `400 Bad Request` and a per-violation list.
3. **Claim approve** — re-evaluate; if a hard violation
   exists, the approval is blocked with the same 400
   response.

#### Scenario: Activated-after-submit blocks approval

- **WHEN** a claim is submitted while no policies exist
- **AND WHEN** the operator then inserts a hard
  `category_cap` policy
- **AND WHEN** the user posts `approve`
- **THEN** the response is 400 with the cap-violation
  message.

#### Scenario: Soft warning does not block

- **WHEN** the policy is `severity=soft`
- **THEN** the submit and approve proceed normally; the
  page surfaces an advisory warning banner instead of
  returning 4xx.

### Requirement: Policy Audit

Creating, toggling, or deleting a policy MUST write an audit
row (`policy.create`, `policy.toggle`, `policy.delete`)
with the `config_json` in metadata.

#### Scenario: Policy create writes an audit row

- **WHEN** the operator inserts a policy
- **THEN** an `audit_entries` row is written with
  `action='policy.create'`, `entity_type='reimbursement_policy'`.

### Requirement: Approval Policies

A ledger SHALL support zero or more `reimbursement_approval_policies`,
each with:

- `id`, `ledger_id`, `name`,
- `min_amount` (decimal ≥ 0),
- `approver_role` (one of the existing `Role` enum values:
  `Admin | Accountant`),
- `level` (positive integer, 1 = first approver).

Policies SHALL be ordered by `min_amount` ascending. For a claim
whose `total = amount + tax_amount` summed across lines, the
set of required approval levels MUST be computed by walking the
ordered policy list and collecting every policy whose
`min_amount <= total`. If no policy matches, the claim MUST
require level 1 by default (preserving the v1 behaviour).

#### Scenario: Two-level policy fires

- **WHEN** the ledger has two policies:
  P1 `min_amount=0, level=1, role=Admin`,
  P2 `min_amount=5000, level=2, role=Accountant`
- **AND WHEN** a claim is submitted with `total=300.00`
- **THEN** the claim requires only level 1 (P1).

#### Scenario: Claim above second threshold requires two levels

- **WHEN** the same ledger and policies exist
- **AND WHEN** a claim is submitted with `total=12000.00`
- **THEN** the claim requires both level 1 (Admin) and level
  2 (Accountant).

### Requirement: Multi-Level State Machine

The `submitted → approved` transition SHALL be extended to
`submitted → partially_approved → fully_approved`. A claim in
`partially_approved` has at least one (but not all) required
level approvals recorded. Authors can view but not edit a
`partially_approved` claim.

`POST /ledgers/{id}/reimbursements/{claim_id}/approve?level=N`
SHALL record one level's approval. After each approve the system
SHALL re-evaluate the required levels:

- If all required levels are now recorded, the claim MUST
  transition to `fully_approved` and the GL posting from the
  base `expense-reimbursement` spec fires.
- Otherwise the claim MUST remain in `partially_approved`.

`approve` MUST be idempotent per level: a second approve at the
same level by the same user is a no-op (`200 OK` with no
state change); by a different user it is also a no-op (already
recorded).

#### Scenario: Claim above second threshold escalates

- **WHEN** a claim requiring L1 + L2 is submitted, an Admin
  user calls `/approve?level=1`, and an Accountant user calls
  `/approve?level=2`
- **THEN** after the second call the claim status is
  `fully_approved` and the GL posting runs exactly once (not
  twice).

#### Scenario: Single-level claim behaves as before

- **WHEN** a claim with `total=300.00` (only L1 required) is
  submitted and an Admin calls `/approve?level=1`
- **THEN** the claim transitions straight to `fully_approved`
  and GL posts — identical behaviour to the base spec.

### Requirement: Approver Discovery

The claim show page SHALL list each required level with a
status badge (`pending | approved`), the approver's name
(once approved), and the timestamp. The author's view shows
only the aggregate status.

#### Scenario: Author sees aggregate status only

- **WHEN** the author opens a `partially_approved` claim
- **THEN** the page shows the badge "Awaiting 1 more approval"
  but does not list approver names.

#### Scenario: Admin sees level breakdown

- **WHEN** a user with role=Admin opens a `partially_approved`
  claim
- **THEN** the page shows level 1 approved by Alice at
  2026-08-14 12:00; level 2 pending.

### Requirement: Approver Cannot Be Author

If a single-operator ledger has only one user, the
self-approve flow from the base spec (`confirm_self_approve=1`)
is preserved. Otherwise, a level's `approver_id` MUST differ
from the claim's `author_id`; the handler returns `403
Forbidden` if violated.

#### Scenario: Author cannot self-approve in shared ledger

- **WHEN** the claim's author is Alice and she tries to
  `/approve?level=1`
- **AND WHEN** there is another user (Bob) with `role=Admin`
  on the ledger
- **THEN** the response is `403 Forbidden` with body
  `Authors cannot approve their own claims.`.

