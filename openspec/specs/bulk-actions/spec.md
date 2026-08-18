# bulk-actions Specification

## Purpose
TBD - created by archiving change u3-bulk-actions. Update Purpose after archive.
## Requirements
### Requirement: Selection

MUST allow selecting one or more rows via checkboxes; a select-all checkbox selects every row on the current page.

#### Scenario: Select all

- **WHEN** the user clicks the header checkbox
- **THEN** every visible row is selected.

### Requirement: Bulk Tag

MUST allow adding a tag to all selected transactions.

#### Scenario: Add tag

- **WHEN** the user selects 3 rows and types 'personal'
- **THEN** all 3 rows gain the tag.

### Requirement: Bulk Untag

MUST allow removing a tag from all selected transactions.

#### Scenario: Remove tag

- **WHEN** the user selects 3 rows with 'personal' and clicks Remove tag 'personal'
- **THEN** all 3 rows lose the tag.

### Requirement: Bulk Assign Contact

MUST allow assigning a contact to all selected transactions.

#### Scenario: Contact

- **WHEN** the user selects 3 rows and picks a contact
- **THEN** all 3 rows have `contact_id` set.

### Requirement: Bulk Delete

MUST use reversing entries (never destructive delete).

#### Scenario: Bulk delete

- **WHEN** the user deletes 3 rows
- **THEN** 3 reversal transactions are created; originals are preserved; an audit row records the bulk action.

### Requirement: Limit

MUST refuse more than 500 rows in a single bulk action.

#### Scenario: Limit

- **WHEN** the user selects 501 rows
- **THEN** 422.

