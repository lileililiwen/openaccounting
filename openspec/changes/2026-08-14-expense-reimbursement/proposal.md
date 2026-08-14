# Add Expense Reimbursement Module

## Why

A first-principles audit found that **expense reimbursement** is the
canonical SMB-defining workflow that turns a "personal bookkeeping
tool" into a "micro-business bookkeeping tool." It is entirely
absent from openaccounting today: `grep -i 'reimburs\|报销'` over
`src/` and `templates/` returns zero hits; there is no module, no
migration, no handler.

The closest peers that ship this workflow:
- **Akaunting** — first-class `Expenses` module with claim + line
  items + approval.
- **ERPNext / Frappe HRMS** — `Expense Claim` doctype with
  approval_status + sanction flow + GL posting.
- **Expensify / Zoho Expense / SAP Concur / Rydoo** — all converge
  on `draft → submitted → approved → paid` with role-based actions.

Reference docs: `docs.frappe.io/hr/expense-claim`,
`github.com/frappe/hrms`, `help.expensify.com/.../Expense-and-Report-Actions.html`,
`sap.com/.../approval-routing`.

## What Changes

- New DB migration `0019_add_reimbursement.sql` with tables
  `reimbursement_claims`, `reimbursement_lines`, `reimbursement_events`.
- New module `src/domain/reimbursement.rs` (pure data) +
  `src/handlers/reimbursement.rs` (HTTP) +
  `src/templates/reimbursement.rs`.
- New HTTP routes:
  - `GET  /ledgers/{id}/reimbursements`
  - `GET  /ledgers/{id}/reimbursements/new`
  - `POST /ledgers/{id}/reimbursements`           (create draft)
  - `GET  /ledgers/{id}/reimbursements/{claim_id}`
  - `POST /ledgers/{id}/reimbursements/{claim_id}/lines`
  - `POST /ledgers/{id}/reimbursements/{claim_id}/submit`
  - `POST /ledgers/{id}/reimbursements/{claim_id}/approve`
  - `POST /ledgers/{id}/reimbursements/{claim_id}/reject`
  - `POST /ledgers/{id}/reimbursements/{claim_id}/pay`
- Default seeded accounts on first reimbursement: `Employee
  Payable` (LIABILITY, subtype `employee_payable`), `Employee
  Advance` (ASSET, subtype `employee_advance`).
- New audit events: `reimbursement.create`, `.submit`,
  `.approve`, `.reject`, `.pay`.
- New nav entry "Reimbursements" under ledger nav.

## Capabilities

### New Capabilities

- `expense-reimbursement` — claim entity, line items, state
  machine, GL posting, receipt attachment.

### Modified Capabilities

- `bookkeeping` — add two new account subtypes:
  `employee_payable` (LIABILITY) and `employee_advance` (ASSET).

## Impact

- **New files:**
  - `migrations/0019_add_reimbursement.sql`
  - `src/domain/reimbursement.rs`
  - `src/handlers/reimbursement.rs`
  - `src/templates/reimbursement.rs`
  - `templates/reimbursements/list.html`
  - `templates/reimbursements/new.html`
  - `templates/reimbursements/show.html`
  - `tests/integration/reimbursement.rs`
  - `tests/fixtures/reimbursement/` (3 fixtures)
- **Modified files:**
  - `src/main.rs` — wire 9 new routes
  - `src/handlers/mod.rs` — `pub mod reimbursement;`
  - `src/domain/ledger.rs::default_chart_of_accounts` — add the
    two new accounts
  - `migrations/0002_default_chart_of_accounts.sql` — extend
    the seeded accounts list (only applies to new ledgers)
  - `templates/partials/_nav.html` — add "Reimbursements" link
  - `openspec/specs/bookkeeping/spec.md` — add subtype rows to
    `Account Types and Normal Direction` (no new behavior)

## Non-Goals

- Multi-level / threshold-based approval routing (post-v1.5).
- Per-diem and mileage calculators (post-v1.5).
- OCR of receipt images (separate change).
- Policy engine (category caps, receipt-required thresholds).
- Intercompany / cross-ledger reimbursement (single ledger only).
