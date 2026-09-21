## Summary

<!-- One paragraph: what changes and why. Link the OpenSpec change
for non-trivial work. -->

## OpenSpec

<!-- e.g. `release-readiness`, or "docs-only / trivial — no change needed". -->

- Change:
- Tasks completed:

## Verification

<!-- Paste or summarize the required evidence. -->

- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --features test-support --all-targets -- -D warnings` clean
- [ ] `cargo test --features test-support` passes
- [ ] Docs-consistency checks pass (`check_tbd_markers`, `check_identity`,
      `check_doc_paths`, `check_roadmap_entries`, `generate_erd --check`)
- [ ] `openspec validate --all --strict` passes
- [ ] CHANGELOG Unreleased updated (user-facing changes)

## Risk / rollback

<!-- Migrations: reversible? API: shape change? Docs: links verified? -->
