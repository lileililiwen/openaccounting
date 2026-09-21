current_spec:

# OpenAccounting handoff

## State

This checkout contains an implemented v0.1-alpha Rust/PostgreSQL application
plus seven active OpenSpec packages. The active packages are proposals and
delegated implementation work; their presence does not prove that the
features are implemented, tested, deployed, or release-ready.

The current package is `ops-hardening` because it establishes the
disclosure process, backup targets and restore evidence, RTO/RPO,
redacted tracing, rate limits, and release attestations needed before
accounting-control depth.

## Completed: openapi-sdk (2026-09-21, commit 9f43db3)

Archived as `openspec/changes/archive/2026-09-21-openapi-sdk/`;
canonical spec `openapi-sdk` created. Evidence: `openspec validate
openapi-sdk --strict` valid; `cargo fmt --check` clean; clippy
profile byte-identical to pre-change dirty HEAD (zero new lints;
repo-wide `-- -D warnings` blocked by pre-existing src/ lints);
`cargo test --features test-support` 17/17 new integration tests
in `tests/integration/openapi_sdk.rs` pass (idempotency store /
replay / 24 h expiry / fingerprint-mismatch 422; route inventory
asserting every src/api `.route("…")` is documented in
docs/openapi.yaml; event catalog asserting EVENT_TYPES from
src/jobs/events.rs appear in docs/event-catalog.md; signed
incoming-event intake enqueues automation_action and rejects
bad signature 401 + unknown type 400; owner-only rule CRUD
via web form; E2E rule → transaction.posted → webhook_delivery
job chain; PATCH ledgers / accounts; POST transactions/reverse;
GET /api/openapi.yaml returns 200 text/yaml). API router moved
to the public axum layer (was incorrectly nested under
login_required + CSRF, causing bearer-only clients to be
redirected to /login — `tests/http_coverage.rs
api_endpoint_without_token_is_unauthorized` reproducibly fails on
pristine HEAD). All 17 new tests green; full parallel integration
run still shows the pre-existing date-sensitive
`automation_platform::scheduler_posts_due_templates_exactly_once_per_due_date`
flake plus the 6 pre-existing pristine-HEAD failures
(2 date-sensitive scheduler/recurring, 2 docs-lint specs,
role_enforcement invoice, recurring_invoices), none of which my
change touches. Migration 0059 reversibility verified by up/down
on scratch DB before archive.

## Completed: pro-close-controls (2026-09-21)

Archived as `openspec/changes/archive/2026-09-21-pro-close-controls/`;
canonical spec `period-hard-close` created. Evidence: `openspec
validate period-hard-close --strict` valid; `cargo fmt --check`
clean; clippy warning profile byte-identical to pristine HEAD
(zero new lints; repo-wide `-- -D warnings` still blocked by
pre-existing src/ lints); `cargo test --features test-support` lib
270/270, integration 432 passed including 8 new
pro-close-controls tests plus 2 new domain unit tests (full
parallel run: 4 failures reproduced on pristine HEAD — 2
date-sensitive scheduler/recurring, docs-lint specs,
role_enforcement invoice — plus 1 ocr parallel flake passing in
isolation and on retry). Migration 0058 up/down/up applied cleanly
on a scratch DB (reversible); ERD regenerated (82 tables).
Deviations recorded in the archived tasks.md: reopen success is a
303 redirect (not 200); auditor export proven via export.json
(report CSV export stays owner-only). Next is `openapi-sdk` per
`ROADMAP.md` order.

## Completed: accounting-dimensions (2026-09-21, commit dbcc529)

Archived as `openspec/changes/archive/2026-09-21-accounting-dimensions/`;
canonical spec `accounting-dimensions` created. Evidence: `openspec
validate accounting-dimensions --strict` valid; `cargo fmt --check`
clean; clippy zero new lints (repo-wide `-- -D warnings` still blocked by
pre-existing src/ lints); `cargo test --features test-support` lib 268/268
single-threaded, integration 425 passed including 8 new
accounting-dimensions tests plus 6 new domain unit tests (4
parallel-run failures all reproduced on pristine HEAD: 2 date-sensitive
scheduler/recurring, TBD purpose in archived ops-hardening spec,
role_enforcement). Also fixed two latent bugs the new tests exposed:
inventory purchase/adjust wrote to nonexistent `postings.currency`
(now removed) and cash lookup used unmatched `subtype='cash'` (now
name-convention with fallback); migration 0057 up applied via test runs,
ERD regenerated (79 tables). Next is `pro-close-controls` per `ROADMAP.md`
order.

## Completed: statement-reconciliation (2026-09-21, commit 0c2097e)

Archived as `openspec/changes/archive/2026-09-21-statement-reconciliation/`;
canonical spec `statement-reconciliation` created. Evidence: `openspec
validate statement-reconciliation --strict` valid; `cargo fmt --check`
clean; clippy zero new lints (repo-wide `-- -D warnings` still blocked by
pre-existing src/ lints, none in new files); `cargo test --features
test-support` lib 262/262 single-threaded, integration 414 passed
including 7 new statement-reconciliation tests (unit 1.1-1.3/1.5 pass;
parallel-run failures reproduced on pristine HEAD or passing in isolation:
2 date-sensitive scheduler/recurring, TBD purpose in archived ops-hardening
spec, role_enforcement, plus parallel flakes in backup/ocr; ERD staleness
from migration 0056 fixed via `generate_erd.py`); migration 0056
up/down applied and reverted on the dev DB (reversible). Next is
`accounting-dimensions` per `ROADMAP.md` order.

## Completed: ops-hardening (2026-09-21, commit a6a51ac)

Archived as `openspec/changes/archive/2026-09-21-ops-hardening/`;
canonical spec `ops-hardening` created. Evidence: `openspec validate
ops-hardening --strict` valid; all script checks OK
(`check_security_contact`, `check_doc_commands` added and CI-wired);
`cargo fmt --check` clean; clippy error profile byte-identical to
pristine HEAD (zero new lints; repo-wide clean blocked by
pre-existing src/ lints); `cargo test --features test-support` lib
251/251 single-threaded, integration 411/414 (8 new ops-hardening
tests pass; 3 failures are date-sensitive scheduler/recurring tests
reproduced on pristine HEAD). Test-DB environment repairs only
(stale shared-DB migration gap 18→55 done last cycle; one stale
login_attempts cleanup). Next is `statement-reconciliation` per
`ROADMAP.md` order.

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

Implement and verify `openapi-sdk` from its own tasks. Keep the active
queue order in `ROADMAP.md`; do not infer completion from documentation or
strict structural validation alone.
