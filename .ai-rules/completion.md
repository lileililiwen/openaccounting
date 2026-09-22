# Completion rules

A change is DONE only when all applicable conditions below have evidence:

- every requirement and observable scenario is implemented or explicitly
  rejected in the change boundary;
- migrated callers, routes, persistence, authorization, validation, and
  compatibility behavior have been rechecked;
- required unit, integration, property, HTTP, E2E, accessibility, security,
  or migration tests are present and passing;
- `node scripts/check-openspec-change-names.mjs` passes;
- applicable local format, clippy, test, documentation, and Gate checks pass;
- `openspec validate --all --strict` passes and the change is archived without
  `--skip-specs`;
- no `current_spec:` placeholder or completed/archived change remains active;
- the final review covers the original BFS impact map and records exact
  commands and results in `HANDOFF.md`.

Build success, test success, strict validation, documentation completion, and
skeleton readiness are not by themselves DONE. Do not claim runtime,
deployment, security, accessibility, backup, or release completion without
the corresponding evidence.

Incomplete or blocked work must be reported as incomplete or blocked with the
exact failed command, boundary, and next action. CI is a second verification
layer, not the first place a change is checked.

Each completed OpenSpec spec/change requires exactly two commits: first the
implementation/tests/archive commit, then a HANDOFF.md-only pointer/evidence
commit.
