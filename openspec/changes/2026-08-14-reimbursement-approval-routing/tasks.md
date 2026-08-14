# Reimbursement Approval Routing — Tasks

## 1. Testing

- [ ] 1.1 Unit: `required_levels` empty → `[1]`.
- [ ] 1.2 Unit: `required_levels` with two matching policies.
- [ ] 1.3 Unit: `recorded_levels` is sorted distinct.
- [ ] 1.4 Integration: `http_claim_above_threshold_requires_two_levels`.
- [ ] 1.5 Integration: `http_two_admin_approves_first_level_only`
      — claim stays `partially_approved`.
- [ ] 1.6 Integration:
      `http_two_levels_approved_moves_to_fully_approved_and_posts_gl`
      — GL posts exactly once.
- [ ] 1.7 Integration: `http_author_cannot_self_approve_in_shared_ledger`
      — 403 with documented body.
- [ ] 1.8 Integration: `http_admin_sees_level_breakdown`,
      author sees aggregate only.
- [ ] 1.9 Property: `prop_required_levels_sorted_distinct` for
      1000 random totals.

## 2. Implementation

- [ ] 2.1 Migration `0022_add_approval_routing.sql`.
- [ ] 2.2 `src/domain/approval_routing.rs` —
      `required_levels`, `recorded_levels`, `eligible_approvers`.
- [ ] 2.3 Extend `ClaimStatus` enum in
      `src/domain/reimbursement.rs` with
      `PartiallyApproved`, `FullyApproved` (the latter
      replaces `Approved` semantically but the existing
      `Approved` value stays for backward compat — see the
      migration).
- [ ] 2.4 Update `src/handlers/reimbursement.rs::approve`
      handler — accept `?level=N`, record step, recompute
      final status.
- [ ] 2.5 `src/handlers/approval_policies.rs` — `list`,
      `new_page`, `create`, `delete`.
- [ ] 2.6 `src/templates/approval_policies.rs`.
- [ ] 2.7 Templates
      `templates/approval_policies/{list,new}.html`.
- [ ] 2.8 Update `templates/reimbursements/show.html` —
      show level breakdown for approvers, aggregate for
      authors.
- [ ] 2.9 `src/main.rs` — 4 new routes for policies +
      update the approve route signature.
- [ ] 2.10 Add `templates/partials/_nav.html` "Approval
      policies" link.

## 3. Validation

- [ ] 3.1 `openspec validate reimbursement-approval-routing` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: invite a second user with role=Admin,
      create a 12,000.00 claim, approve as the second user at
      level 1, approve as Admin at level 2 — verify exactly
      one GL transaction is posted.
- [ ] 3.6 `openspec archive reimbursement-approval-routing`.
