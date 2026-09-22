# ux-a11y Specification

## Purpose

The UX / accessibility / mobile-promise capability is the user-facing
contract that the web UI is usable by keyboard-only and screen-reader
users, that missing translations are caught before release, and that
the mobile story is stated honestly. The repo carries a dated
WCAG 2.2 AA audit report with findings, severity, and fix status; a
docs-lint gate blocks any release-notes claim of accessibility
conformance while a P1 finding remains open. HTMX partial swaps carry
`data-htmx-focus` and `data-htmx-announce` markers; a single
`static/js/a11y.js` helper moves focus to the named element and speaks
via a lazily-created `aria-live` region. Charts wrap their SVG in
`<figure role="img" aria-label="…">` plus a visually-hidden `<table>`
data summary. A per-language missing-key percentage is published on
every CI run and a day-1 locale exceeding 5 % missing keys fails the
build. The mobile promise is stated identically in `README.md` and
`mobile/README.md` and linted by a dedicated script; the Capacitor
shell was retired in favour of the installable PWA shipped from the
binary. Out of scope: a native mobile shell and an offline write queue.
## Requirements
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

