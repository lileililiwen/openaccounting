## 1. Testing

- [x] 1.1 Unit: docs-lint detects TBD markers, stale identity strings, and broken doc paths on fixtures.
- [x] 1.2 Integration: roadmap-entry check flags a fixture change lacking a ROADMAP entry.
- [x] 1.3 Integration: ERD generation from fixture migrations matches checked-in diagram.

## 2. Implementation

- [x] 2.1 ROADMAP.md with Now/Next/Later, non-goals, and v1.0 exit gates.
- [x] 2.2 Rewrite TBD purposes in project-governance, release-integrity, test-coverage specs.
- [x] 2.3 GOVERNANCE.md plus issue/PR templates and triage SLA badge.
- [x] 2.4 Operator docs: ERD, admin runbook, sizing guide, API reference pointer.
- [x] 2.5 CI: wire TBD, identity, doc-path, and roadmap-entry checks.
- [x] 2.6 CHANGELOG Unreleased refresh with migration notes.

## 3. Validation

- [x] 3.1 `openspec validate release-readiness` passes.
- [ ] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.3 `cargo test --features test-support` passes including docs-lint tests.
