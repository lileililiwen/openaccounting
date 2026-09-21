## ADDED Requirements

### Requirement: Roadmap With Gates

ROADMAP.md SHALL list Now/Next/Later priorities, explicit non-goals, and measurable v1.0 exit gates (close controls, reconciliation, PDF archiving, OpenAPI, backup drill, a11y P1 clear, zero TBD purposes). Merging a feature without a roadmap entry SHALL fail docs-lint.

#### Scenario: Unlisted feature blocked

- **WHEN** a change adds a user-facing capability missing from ROADMAP
- **THEN** docs-lint fails naming the missing entry.

### Requirement: No TBD Purposes

No accepted spec SHALL contain `TBD - created by archiving`. The existing TBD purposes SHALL be rewritten to state the real contract, user, and out-of-scope bounds.

#### Scenario: TBD fails CI

- **WHEN** any spec contains the TBD marker
- **THEN** the consistency check fails.

### Requirement: Changelog Discipline

Every user-facing change SHALL add a CHANGELOG Unreleased entry with migration notes where applicable. Release jobs SHALL fail when the version tag lacks matching notes.

#### Scenario: Missing notes flagged

- **WHEN** a change ships a migration without a CHANGELOG migration note
- **THEN** docs-lint fails.

### Requirement: Governance and Triage

GOVERNANCE.md SHALL define maintainer merge rules and a 5-business-day acknowledge SLA. Issue and PR templates SHALL exist. Stale-identity and broken-path checks from Agents.md section 9 SHALL run in CI.

#### Scenario: Broken doc path fails

- **WHEN** README references a missing file
- **THEN** `scripts/check_doc_paths.py` fails CI.
