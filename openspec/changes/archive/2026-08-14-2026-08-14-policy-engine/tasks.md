# Policy Engine — Tasks

## 1. Testing

- [ ] 1.1 Unit: `evaluate` returns a `CategoryCapOver`
      violation when the daily sum exceeds the cap.
- [ ] 1.2 Unit: `evaluate` returns a `ReceiptMissing`
      violation when `amount >= min_amount` and no receipt.
- [ ] 1.3 Unit: per-diem over-by-X calculation correct.
- [ ] 1.4 Property: `prop_evaluator_is_deterministic` for
      1000 random claim/policy combos.
- [ ] 1.5 Integration: `http_cap_blocks_submit` — 400 on
      submit.
- [ ] 1.6 Integration: `http_receipt_required_blocks_approve`
      — 400 on approve after policy activation.
- [ ] 1.7 Integration: `http_policy_audit_row_written`.
- [ ] 1.8 Integration: `http_soft_warning_does_not_block`.

## 2. Implementation

- [ ] 2.1 Migration `0026_add_reimbursement_policies.sql`.
- [ ] 2.2 `src/domain/policies.rs` — `Policy`, `PolicyKind`,
      `Violation`, `evaluate`.
- [ ] 2.3 `src/handlers/policies.rs` — `list`, `new_page`,
      `create`, `toggle`, `delete`.
- [ ] 2.4 `src/templates/policies.rs`.
- [ ] 2.5 Templates `templates/policies/{list,new}.html`.
- [ ] 2.6 `src/handlers/reimbursement.rs` — call
      `policies::evaluate` on `add_line`, `submit`, `approve`;
      return 400 on hard violations.
- [ ] 2.7 Update `templates/reimbursements/show.html` —
      violations banner.
- [ ] 2.8 `src/main.rs` — 4 new routes.
- [ ] 2.9 Add `templates/partials/_nav.html` "Policies" link.
- [ ] 2.10 Add `policy.create`, `policy.toggle`,
      `policy.delete` to the audit whitelist.

## 3. Validation

- [ ] 3.1 `openspec validate policy-engine` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: create a 200/day meals cap, submit a
      claim with 250/day → 400 with documented body.
- [ ] 3.6 `openspec archive policy-engine`.
