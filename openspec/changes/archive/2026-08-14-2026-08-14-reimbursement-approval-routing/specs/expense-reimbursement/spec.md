# expense-reimbursement Specification (delta)

## ADDED Requirements

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
