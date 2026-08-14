# Add Multi-Level / Threshold-Based Approval Routing

## Why

The current reimbursement state machine (introduced by
`2026-08-14-expense-reimbursement`) supports a single approval
step. SAP Concur, Zoho Expense, and Rydoo all support
**multi-level / threshold-based routing**: claims under ¥500
auto-approve at L1, claims between ¥500 and ¥5000 require
L1 + L2 approval, claims above ¥5000 additionally require a
finance director. Micro-businesses with a single approver don't
care, but the moment a company has a finance controller the
single-step workflow blocks adoption.

Reference: SAP Concur "Approval Routing"
(`help.sap.com/docs/concur-expense/.../approval-routing`),
Zoho Expense "Policies" (`zoho.com/expense/policies`).

## What Changes

- New column `claim_amount_threshold` rows in
  `reimbursement_approval_policies` (new table).
- New table `reimbursement_approval_steps`
  (`claim_id`, `level`, `approver_id`, `status`).
- A claim's required approval levels are computed at submit-time
  by walking the policy list (ordered by `min_amount`) and
  collecting every step whose `min_amount <= claim.total`.
- The state machine extends `submitted → partially_approved →
fully_approved`; `partially_approved` is a transient status
visible to approvers but not to authors.
- A `POST /ledgers/{id}/reimbursements/{claim_id}/approve?level=N`
  endpoint records a level-specific approval and auto-transitions
  to `fully_approved` when all required levels are done.

## Capabilities

### Modified Capabilities

- `expense-reimbursement` — adds the `Approval Policies`
  requirement and the level-aware state transition.

## Impact

- **New files:**
  - `migrations/0022_add_approval_routing.sql`
  - `src/domain/approval_routing.rs`
  - `src/handlers/approval_policies.rs`
  - `src/templates/approval_policies.rs`
  - `templates/approval_policies/{list,new}.html`
  - `tests/integration/approval_routing.rs`
- **Modified files:**
  - `src/handlers/reimbursement.rs` — `approve` becomes
    level-aware.
  - `src/domain/reimbursement.rs` — `ClaimStatus` gains
    `partially_approved`.
  - `templates/reimbursements/show.html` — show approval chain
    progress.

## Non-Goals

- Dynamic approver resolution (e.g. "current CFO" lookup).
  Approvers are picked by static role assignment.
- Email / push notifications to approvers (separate change).
- Delegation (approver-out-of-office). Separate change.
