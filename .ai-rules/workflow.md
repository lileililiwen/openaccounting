# Change workflow

This repository uses OpenSpec for non-trivial changes. The workflow is
BFS → DFS → BFS and is required for code, migrations, API behavior, UI flows,
security, operations, and release changes.

## 1. BFS analysis

Before editing, inspect the selected proposal/design/spec/tasks and map the
requirements to domain types, handlers, routes, templates, migrations,
callers, imports/exports, tests, authorization, compatibility, observability,
and release concerns. Record unresolved boundaries in the change design.

Run `node scripts/check-openspec-change-names.mjs` before using
`openspec status --change` or `openspec instructions --change`.

## 2. Structural pass

Update domain types, contracts, signatures, DTOs, events, dependency wiring,
callers, migration shape, and test skeletons in dependency order. Compilation
or a passing skeleton is `SKELETON_READY`, not completion.

## 3. DFS implementation

Write the test for one observable requirement/scenario first, then implement
that scenario through the affected layers: domain, application/reporting,
storage/infrastructure, handler/API, and template/UI. Preserve the balanced
posting invariant, authorization, exact Decimal arithmetic, and reversible
migrations.

## 4. BFS verification

Re-check every requirement, scenario, caller, persistence path, route,
authorization rule, validation error, log/event, concurrency case,
compatibility boundary, placeholder, and required test. Run local checks
before archive or completion claims.

## OpenSpec lifecycle

Use letter-first lowercase kebab-case names. The lifecycle is:

```text
openspec list → select one change → set HANDOFF current_spec
→ test/implement → local verify → strict validate → archive
→ commit 1 → update HANDOFF evidence/pointer → commit 2 → stop
```

Each completed OpenSpec spec/change requires exactly two commits: first the
implementation/tests/archive commit, then a HANDOFF.md-only pointer/evidence
commit.

Keep exactly one live `current_spec:` line in `HANDOFF.md`. Set it before
work, advance it only after archive and commit 1, and remove it in the final
handoff when no active change remains. Never use `none` or `TBD`.

## Verification baseline

```sh
node scripts/check-openspec-change-names.mjs
python3 scripts/check_doc_paths.py
cargo fmt -- --check
cargo clippy --features test-support --all-targets -- -D warnings
cargo test --features test-support
openspec validate --all --strict
```

Use CI commands from `.github/workflows/ci.yml` when a change touches those
surfaces. Runtime and deployment evidence must be collected separately.
