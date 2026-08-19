# draft-transactions Specification (delta)

## ADDED Requirements

### Requirement: Save as Draft

MUST allow saving a transaction with `kind='draft'`; the draft MUST be persisted but MUST NOT affect any report.

#### Scenario: Save draft

- **WHEN** the user clicks 'Save draft'
- **THEN** the transaction exists with kind=draft; trial balance is unchanged.

### Requirement: Promote

MUST allow promoting a draft to posted; the audit log MUST record the promotion; reports MUST include the transaction from the promoted date forward.

#### Scenario: Promote

- **WHEN** the user clicks 'Post' on a draft
- **THEN** kind becomes 'posted'; report totals update.

### Requirement: Delete Draft

MUST allow deleting a draft without creating a reversal.

#### Scenario: Delete draft

- **WHEN** the user deletes a draft
- **THEN** no audit reversal; the row is gone.

### Requirement: Drafts Page

MUST show drafts on a dedicated `/ledgers/{id}/drafts` page.

#### Scenario: Drafts page

- **WHEN** the user visits the drafts page
- **THEN** all drafts are listed.

### Requirement: Validation Skipped for Drafts

MUST skip the period-close check for drafts; drafts in closed periods are allowed.

#### Scenario: Draft in closed period

- **WHEN** the user saves a draft dated 2024
- **THEN** the draft is saved even though 2024 is closed.
