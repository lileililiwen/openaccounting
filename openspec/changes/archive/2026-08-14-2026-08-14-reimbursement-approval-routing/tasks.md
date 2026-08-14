# Reimbursement Approval Routing — Tasks

## 1. Testing

- [x] 1.1 Unit: `required_levels` empty → `[1]`.
- [x] 1.2 Unit: `required_levels` with two matching policies.
- [x] 1.3 Unit: `recorded_levels` is sorted distinct.
- [x] 1.4 Integration: `http_claim_above_threshold_requires_two_levels`.
- [x] 1.5 Integration: first Admin approve leaves the claim
      `partially_approved` (covered by
      `http_claim_above_threshold_requires_two_levels`).
- [x] 1.6 Integration:
      `http_two_levels_approved_moves_to_fully_approved_and_posts_gl`
      — GL posts exactly once.
- [x] 1.7 Integration: `http_author_cannot_self_approve_in_shared_ledger`
      — 403 with documented body.
- [x] 1.8 Integration: `http_admin_sees_level_breakdown`,
      author sees aggregate only.
- [x] 1.9 Property: `prop_required_levels_sorted_distinct` for
      1000 random totals.

## 2. Implementation

- [x] 2.1 Migration `0027_add_approval_routing.sql`.
      (Design said `0022`, but 0022-0025 are skipped external-service
      migrations and `0026_add_reimbursement_policies.sql` exists, so the
      next available migration number is 0027.)
- [x] 2.2 `src/domain/approval_routing.rs` —
      `required_levels`, `recorded_levels`, `eligible_approvers`,
      `approver_role_for_level`, `ledger_user_count`.
- [x] 2.3 Extend `ClaimStatus` enum in
      `src/domain/reimbursement.rs` with
      `PartiallyApproved`, `FullyApproved` (the latter
      replaces `Approved` semantically but the existing
      `Approved` value stays for backward compat — see the
      migration).
- [x] 2.4 Update `src/handlers/reimbursement.rs::approve`
      handler — accept `?level=N`, record step, recompute
      final status.
- [x] 2.5 `src/handlers/approval_policies.rs` — `list`,
      `new_page`, `create`, `delete`.
- [x] 2.6 `src/templates/approval_policies.rs`.
- [x] 2.7 Templates
      `templates/approval_policies/{list,new}.html`.
- [x] 2.8 Update `templates/reimbursements/show.html` —
      show level breakdown for approvers, aggregate for
      authors.
- [x] 2.9 `src/lib.rs` — 4 new routes for policies +
      update the approve route signature (routes live in
      `src/lib.rs`, not `src/main.rs`).
- [x] 2.10 Add `templates/partials/_nav.html` "Approvals"
      and "Expenses" links.

## 3. Validation

- [x] 3.1 `openspec validate 2026-08-14-reimbursement-approval-routing`
      passes.
- [x] 3.2 `cargo fmt --check` clean (only pre-existing diffs remain;
      my new files are formatted).
- [x] 3.3 `cargo clippy --all-targets` — no new warnings from touched
      files (the `type_complexity` in `load_claim` is pre-existing).
- [x] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: invite a second user with role=Admin,
      create a 12,000.00 claim, approve as the second user at
      level 1, approve as Admin at level 2 — verify exactly
      one GL transaction is posted.
- [x] 3.6 `openspec archive 2026-08-14-reimbursement-approval-routing`.

## Deviations from design

- Migration is `0027` not `0022` (0022-0025 are skipped).
- There is no `Role` enum / `ledger_shares` table. `eligible_approvers`
  maps `Admin` to the ledger owner or a global-admin user with edit
  access, and `Accountant` to any owner/editor member. Viewers are
  never eligible (base spec).
- The self-approve rule uses `ledger_user_count > 1` (owner + members)
  to decide single-operator vs shared ledger.
- `approve` and the claim `show` page use `ledgers::ensure_access`
  (owner/editor/viewer) rather than `ensure_owner`, because the delta
  spec requires approvers who are not owners to view and approve
  claims. All other reimbursement handlers keep `ensure_owner`.
- Idempotent `200` for a repeated level is checked before eligibility
  so a second approval at an already-recorded level is a no-op for
  any user, per the delta spec.
- `total` used for policy thresholds is `amount + tax_amount` per line,
  matching the delta spec.
