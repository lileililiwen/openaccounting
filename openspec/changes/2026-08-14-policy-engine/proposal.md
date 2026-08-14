# Add Reimbursement Policy Engine

## Why

A policy engine is what turns a reimbursement tool from "form
to fill" into "control system." Zoho Expense, SAP Concur, and
Rydoo all enforce policies: per-category caps, receipt-required
thresholds, per-diem rates, blocked merchants. Without policies,
the approver has no defense against accidental or fraudulent
overspend.

The current reimbursement module
(`2026-08-14-expense-reimbursement`) accepts any claim. This
change adds a configurable policy layer that:

1. Validates each line against its category's rules.
2. Computes per-diem allowance for travel lines.
3. Blocks approval of any claim that violates an active policy,
   and surfaces the violation to the author and approver.

## What Changes

- New table `reimbursement_policies`
  (`id, ledger_id, name, kind, config_json, severity,
   is_active, created_at`).
- Three `kind` values:
  - `category_cap`: max amount per line per category per day
    (`config_json = {"category":"meals","max_per_day":200.00}`).
  - `receipt_required`: lines at or above N require a
    receipt (`config_json = {"min_amount":50.00}`).
  - `per_diem`: destination-based daily allowance
    (`config_json = {"destination":"berlin","daily_rate":150.00,
    "currency":"EUR"}`).
- Policy evaluation runs:
  - on line add (advisory warnings to the author),
  - on submit (blocking violations return 400),
  - on approve (re-evaluation; if a policy was activated between
    submit and approve, the approval is blocked with the
    violations listed).
- New routes:
  - `GET  /ledgers/{id}/policies`
  - `POST /ledgers/{id}/policies`
  - `POST /ledgers/{id}/policies/{policy_id}/toggle`
  - `POST /ledgers/{id}/policies/{policy_id}/delete`
- `severity = "hard"` policies block; `"soft"` policies warn but
  allow.

## Capabilities

### Modified Capabilities

- `expense-reimbursement` — adds the `Policy Engine` requirement.

## Impact

- **New files:**
  - `migrations/0026_add_reimbursement_policies.sql`
  - `src/domain/policies.rs`
  - `src/handlers/policies.rs`
  - `src/templates/policies.rs`
  - `templates/policies/{list,new}.html`
  - `tests/integration/policies.rs`
- **Modified files:**
  - `src/handlers/reimbursement.rs` — call `evaluate_policies`
    on submit + approve.
  - `src/domain/reimbursement.rs` — `PolicyViolation` enum
    threaded through the return type.
  - `templates/reimbursements/show.html` — show violations
    banner.
  - `templates/partials/_nav.html` — add "Policies" link.

## Non-Goals

- Tax-specific policies (e.g. "no meals over $75 with alcohol
  purchased"). Category-level caps only.
- Geo-fenced per-diem tables. v1 uses a flat rate per
  destination.
- Policy inheritance across ledgers (multi-entity). Each
  ledger defines its own policies.
