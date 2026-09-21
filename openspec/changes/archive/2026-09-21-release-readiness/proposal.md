# Proposal: Roadmap, spec hygiene, and contributor readiness

## Why

`openspec/changes/` contains only `archive/` with no active proposals, so planning appears stalled. Several accepted specs still carry `TBD - created by archiving` purposes (`project-governance`, `release-integrity`, `test-coverage`), violating the repo's own documentation-consistency rule. CHANGELOG Unreleased has drifted from 0.1.0, there is no ROADMAP with v1.0 exit criteria, and contributor governance (triage SLA, decision process) plus operator docs (ERD, admin runbook, sizing) are missing. New contributors cannot tell what is next or done.

## What Changes

- ROADMAP.md with prioritized backlog, explicit non-goals, and measurable v1.0 exit gates.
- Rewrite all TBD spec purposes with real contracts.
- CHANGELOG discipline: every user-facing change lands under Unreleased with migration notes.
- GOVERNANCE.md plus issue/PR templates and triage SLA.
- Operator and contributor docs: data-model ERD, admin runbook, API reference pointer, sizing guide.

## Capabilities

### New Capabilities
- `release-readiness`: roadmap, v1.0 gates, governance, triage SLA, ERD, runbook, changelog discipline.

### Modified Capabilities
- `project-governance`: TBD purpose is replaced with the real contributor contract.
- `release-integrity`: TBD purpose is replaced with the real reproducible-release contract.
- `test-coverage`: TBD purpose is replaced with the real coverage-gate contract.

## Impact

Affected: `ROADMAP.md`, `GOVERNANCE.md`, `.github/*` templates, `openspec/specs/*/spec.md` purposes, `docs/*` operator guides. Unaffected: runtime code, migrations, API shapes.
