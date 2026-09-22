## ADDED Requirements

### Requirement: Audit Evidence

The repo SHALL contain a dated WCAG 2.2 AA audit report with findings, severity, and fix status. Release notes SHALL NOT claim accessibility conformance while P1 findings remain open.

#### Scenario: Audit gates claims

- **WHEN** a P1 finding is open and release notes claim AA conformance
- **THEN** the docs-lint test fails naming the open finding.

### Requirement: Focus and Announcements

HTMX partial swaps SHALL move focus to the updated region and announce completion via aria-live. Chart containers SHALL expose role=img with a text summary plus a hidden data table.

#### Scenario: Transaction post announces

- **WHEN** a keyboard user posts a balanced transaction via the editor
- **THEN** focus lands on the confirmation region and the announcement text includes the transaction total.

### Requirement: Locale Coverage Gate

CI SHALL publish per-language missing-key percentages; builds SHALL fail when a day-1 language exceeds 5% missing keys.

#### Scenario: Missing keys block release

- **WHEN** German is missing 8% of keys
- **THEN** CI fails with the missing-key list.

### Requirement: Mobile Promise

The repo SHALL EITHER support the Capacitor shell with a receipt-capture flow (photo → document inbox → transaction link) OR remove `mobile/` and document PWA-only install. Both README and mobile README SHALL state the same promise.

#### Scenario: Promise agrees

- **WHEN** docs-lint runs
- **THEN** README non-goals and mobile README status match exactly.
