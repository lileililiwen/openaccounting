# empty-states Specification

## Purpose
TBD - created by archiving change u9-empty-states. Update Purpose after archive.
## Requirements
### Requirement: Empty Partial

MUST render a dedicated empty partial for each list view when zero rows exist.

#### Scenario: Empty list

- **WHEN** a fresh user visits /transactions
- **THEN** the empty partial is rendered with the CTA.

### Requirement: CTAs

MUST offer the most relevant next action as a primary button.

#### Scenario: CTA

- **WHEN** no transactions
- **THEN** the CTA is 'Create your first transaction'.

### Requirement: Illustrations

MUST include a small inline SVG illustration for each empty state.

#### Scenario: SVG present

- **WHEN** the page source
- **THEN** an inline SVG is present.

### Requirement: No JS

MUST NOT depend on JavaScript.

#### Scenario: No JS

- **WHEN** JS disabled
- **THEN** the empty state still renders correctly.

