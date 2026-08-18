# saved-searches Specification

## Purpose
TBD - created by archiving change u2-saved-searches. Update Purpose after archive.
## Requirements
### Requirement: Save

MUST allow saving the current filter set as a named search.

#### Scenario: Save

- **WHEN** the user clicks 'Save current view as'
- **THEN** a row is written with name + query string.

### Requirement: Apply

MUST allow clicking a saved search to re-render the list with its filters applied.

#### Scenario: Apply

- **WHEN** the user clicks 'Unpaid invoices > 30 days'
- **THEN** the list loads with the matching query string.

### Requirement: Rename

MUST allow renaming and deleting.

#### Scenario: Delete

- **WHEN** the user deletes a saved search
- **THEN** the row is removed; the list is unchanged.

### Requirement: Per-User

MUST scope saved searches to the user (not to the ledger) so personal workflows don't leak to other users.

#### Scenario: Scope

- **WHEN** user A's saved search is not visible to user B
- **THEN** A's saved list shows; B's does not.

### Requirement: Default

MUST allow setting one saved search as the default landing page.

#### Scenario: Default

- **WHEN** user sets 'Unpaid' as default
- **THEN** visiting /transactions renders the unpaid filter.

