# Expense Reimbursement — Tasks

## 1. Testing

- [ ] 1.1 Unit: `ClaimStatus::can_transition_to` covers the
      full state-machine matrix (allowed + rejected transitions).
- [ ] 1.2 Unit: `format_short_id` is stable for the same uuid.
- [ ] 1.3 Integration: `http_create_draft_claim` — POST → 303,
      claim visible.
- [ ] 1.4 Integration: `http_create_claim_currency_mismatch`
      — 400 with the documented body.
- [ ] 1.5 Integration: `http_add_line_to_claim` — line appears
      in show.
- [ ] 1.6 Integration: `http_add_line_with_non_expense_gl`
      — 400 with the documented body.
- [ ] 1.7 Integration: `http_submit_empty_claim` — 400.
- [ ] 1.8 Integration: `http_reject_without_reason` — 400.
- [ ] 1.9 Integration: `http_approve_idempotent` — second approve
      returns 409.
- [ ] 1.10 Integration: `http_approve_creates_postings` — 2
      postings, balanced.
- [ ] 1.11 Integration: `http_approve_with_recoverable_tax`
      — 3 postings, balanced.
- [ ] 1.12 Integration: `http_approve_atomic_on_failure` —
      claim stays `submitted`, 0 postings.
- [ ] 1.13 Integration: `http_pay_clears_payable` — DR
      Payable / CR Bank, claim status flips to `paid`.
- [ ] 1.14 Integration: `http_payout_must_be_asset_cash_or_bank`
      — 400 on subtype mismatch.
- [ ] 1.15 Integration: `http_advance_netting` — 4 postings,
      `Employee Payable` reduced by the advance amount.
- [ ] 1.16 Integration: `http_viewer_cannot_approve` — 403.
- [ ] 1.17 Integration: `http_filter_by_status` — only the
      requested statuses appear.
- [ ] 1.18 Property: `prop_state_machine_is_acyclic` for 1000
      random walks.

## 2. Implementation

- [ ] 2.1 Migration `0019_add_reimbursement.sql`:
      three tables, indexes, ALTER accounts subtype CHECK.
- [ ] 2.2 Extend `default_chart_of_accounts` in
      `src/domain/ledger.rs` with `Employee Payable` (LIABILITY /
      `employee_payable`) and `Employee Advance` (ASSET /
      `employee_advance`).
- [ ] 2.3 `src/domain/reimbursement.rs` — `Claim`, `Line`,
      `Event`, `ClaimStatus` enum + `can_transition_to`.
- [ ] 2.4 `src/auth/mod.rs` — `Role` enum.
- [ ] 2.5 `src/handlers/ledgers.rs` — `ensure_role` helper
      (sibling to `ensure_owner`).
- [ ] 2.6 `src/domain/transaction.rs` —
      `create_transaction_with_postings` helper.
- [ ] 2.7 `src/handlers/reimbursement.rs` — `list`, `new`,
      `create`, `show`, `add_line`, `submit`, `approve`,
      `reject`, `pay` handlers.
- [ ] 2.8 `src/main.rs` — wire 9 new routes.
- [ ] 2.9 `src/handlers/mod.rs` — `pub mod reimbursement;`.
- [ ] 2.10 `src/templates/reimbursement.rs` — Askama structs
      for list / new / show.
- [ ] 2.11 Templates:
      `templates/reimbursements/{list,new,show}.html`.
- [ ] 2.12 Update `templates/partials/_nav.html` — add
      "Reimbursements" link.
- [ ] 2.13 Extend `src/handlers/documents.rs::upload` with
      optional `claim_line_id` form field.
- [ ] 2.14 Update `openspec/specs/bookkeeping/spec.md`
      `Account Types and Normal Direction` table with the two
      new subtypes.

## 3. Validation

- [ ] 3.1 `openspec validate expense-reimbursement` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: create ledger → claim → line → submit →
      approve → pay, asserting the balance sheet `Employee
      Payable` returns to 0 after payout.
- [ ] 3.6 Manual smoke: invite a second user with `role=viewer`,
      verify they cannot approve.
- [ ] 3.7 `openspec archive expense-reimbursement`.
