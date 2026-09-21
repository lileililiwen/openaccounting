current_spec: ops-hardening

# OpenAccounting handoff

## State

This checkout contains an implemented v0.1-alpha Rust/PostgreSQL application
plus eight active OpenSpec packages. The active packages are proposals and
delegated implementation work; their presence does not prove that the
features are implemented, tested, deployed, or release-ready.

The current package is `ops-hardening` because it establishes the
disclosure process, backup targets and restore evidence, RTO/RPO,
redacted tracing, rate limits, and release attestations needed before
accounting-control depth.

## Completed: release-readiness (2026-09-21, commit 3c40fde)

Archived as `openspec/changes/archive/2026-09-21-release-readiness/`;
canonical specs `release-readiness` (new), `project-governance`,
`release-integrity`, `test-coverage` (purposes rewritten, durable-purpose
requirements added). Evidence: `openspec validate release-readiness
--strict` valid; `check_tbd_markers`, `check_identity`,
`check_doc_paths`, `check_roadmap_entries`, `check_changelog`,
`generate_erd --check` all OK; `cargo fmt --check` clean;
`cargo test --features test-support` lib 243/243, integration 402/405
(new 14 docs-consistency tests pass). Not claimed: repo-wide clippy
clean (104 pre-existing src/ lints), 3 http_coverage + 1 smoke + 3
integration failures and 3 `--all --strict` spec failures, each
reproduced on pristine HEAD via `git stash -u` and unrelated to this
change. Test DB migrated 18→55 and one stale `totp-test` row removed
(environment repair only).

## Start-of-cycle procedure

1. Read this file and run `openspec list`.
2. Run `node scripts/check-openspec-change-names.mjs`.
3. Confirm the selected folder matches the single `current_spec:` pointer.
4. Read its `proposal.md`, `design.md`, `tasks.md`, and capability specs.
5. Implement only that package and its tests, using BFS → DFS → BFS.

## End-of-cycle procedure

Run the applicable project checks, strict OpenSpec validation, and any
runtime/release evidence required by the package. Archive only after local
verification succeeds; never use `--skip-specs`. Update the package tasks as
work is completed.

Each completed OpenSpec spec/change requires exactly two commits: first the
implementation/tests/archive commit, then a HANDOFF.md-only pointer/evidence
commit.

Commit 1 contains implementation, tests, archived change material, and
promoted canonical specs. Then update this file with exact evidence and move
`current_spec:` to the next active package, or remove it when none remains.
Commit 2 contains only `HANDOFF.md`. Stop after commit 2; do not start the
next package or push from this workflow.

## Evidence observed at bootstrap

- `openspec list` reports eight active packages, each at `0/N` tasks.
- The repository has CI workflows for lint, tests, migration reversibility,
  coverage, security, release validation, and mobile builds.
- The source tree, migrations, templates, static assets, and integration tests
  are present.
- Runtime, deployment, security-contact remediation, accessibility, and
  release verification were not re-proven by this documentation update.

## Next action

Implement and verify `release-readiness` from its own tasks. Keep the active
queue order in `ROADMAP.md`; do not infer completion from documentation or
strict structural validation alone.
