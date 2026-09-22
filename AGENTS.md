# OpenAccounting agent guide

OpenAccounting is a Rust 1.95 single-binary Axum application with PostgreSQL,
sqlx migrations, Askama templates, HTMX, and a responsive web UI. The core
domain invariant is balanced double-entry bookkeeping: every transaction's
debits equal its credits.

## Required workflow

- Read `HANDOFF.md`, `ROADMAP.md`, and the selected OpenSpec change before work.
- Non-trivial changes require an OpenSpec change; use lowercase,
  letter-first kebab-case names.
- Use BFS → DFS → BFS: map the impact surface, make the structural pass,
  implement one scenario at a time, then re-check the full surface.
- Run `node scripts/check-openspec-change-names.mjs` before selecting or
  validating a change.
- Keep domain logic independent of HTTP and templates; preserve migration
  reversibility and authorization boundaries.
- Build success, test success, and skeleton completion are not by themselves
  DONE. Runtime, security, accessibility, and release claims need matching
  evidence.

Detailed rules live in [`.ai-rules/workflow.md`](.ai-rules/workflow.md),
[`.ai-rules/completion.md`](.ai-rules/completion.md), and
[`.ai-rules/architecture.md`](.ai-rules/architecture.md).

The machine-readable current change is the single `current_spec:` line in
`HANDOFF.md`; update it only at the start/end of a change cycle.

Each completed OpenSpec spec/change requires exactly two commits: first the
implementation/tests/archive commit, then a HANDOFF.md-only pointer/evidence
commit.

## Verification commands

```sh
node scripts/check-openspec-change-names.mjs
python3 scripts/check_doc_paths.py
cargo fmt -- --check
cargo clippy --features test-support --all-targets -- -D warnings
cargo test --features test-support
openspec validate --all --strict
```

Use the narrowest applicable commands during development and the full set
before completion. Do not claim that a command passed unless it was run.

## Handoff

Follow `HANDOFF.md` for the active delegated implementation queue. Do not
start a second OpenSpec change after completing one; update the handoff and
stop after the required second commit.
