# Reconciliation Automation — Tasks

## 1. Testing

- [ ] 1.1 Unit: `evaluate_predicate` covers each single-key
      predicate true / false.
- [ ] 1.2 Unit: compound predicates are AND-combined.
- [ ] 1.3 Unit: `sql_like_match` handles `%` and `_`.
- [ ] 1.4 Unit: priority tiebreaking returns the
      lowest-numbered matching rule.
- [ ] 1.5 Property: `prop_predicate_is_dnf_of_atoms` for 1000
      random (predicate, line) pairs.
- [ ] 1.6 Integration: `http_categorize_rule_suggestion_renders`.
- [ ] 1.7 Integration: `http_apply_categorize_creates_transaction`.
- [ ] 1.8 Integration: `http_match_rule_pairs_lines`.
- [ ] 1.9 Integration: `http_priority_tiebreak_picks_lower`.
- [ ] 1.10 Integration: `http_rule_create_validates_predicate_types`.

## 2. Implementation

- [ ] 2.1 Migration `0021_add_reconciliation_rules.sql`.
- [ ] 2.2 `src/domain/reconciliation_rules.rs` — `Rule`,
      `Kind`, `Action`, `Predicate`, `evaluate_predicate`.
- [ ] 2.3 `src/handlers/rules.rs` — `list`, `new_page`,
      `create`, `toggle`, `delete`, `apply`.
- [ ] 2.4 `src/templates/rules.rs`.
- [ ] 2.5 Templates `templates/rules/{list,new}.html`.
- [ ] 2.6 Update `templates/reconciliation/page.html` —
      suggestions column.
- [ ] 2.7 Update `src/handlers/reconciliation.rs` — call
      `evaluate_predicate` per rule on page load.
- [ ] 2.8 `src/main.rs` — 6 new routes.
- [ ] 2.9 Add `rule.create`, `rule.toggle`, `rule.delete`,
      `rule.apply` to the audit event whitelist.

## 3. Validation

- [ ] 3.1 `openspec validate reconciliation-automation` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: create a STARBUCKS% categorize rule,
      import an OFX file with one STARBUCKS line → preview
      page shows the suggestion → click Apply → new
      transaction appears.
- [ ] 3.6 `openspec archive reconciliation-automation`.
