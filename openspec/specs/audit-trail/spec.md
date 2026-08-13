# audit-trail Specification

## Purpose
Record who did what, when, and to what. Provides accountability, regulatory compliance (SOX, GAAP), and the ability to investigate issues.

## Requirements

### Requirement: Audit Log Table

The system MUST maintain an `audit_entries` table with at minimum:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID (nullable, for ledger-scoped events)
- `actor_id` UUID (the user who performed the action)
- `action` TEXT (e.g., 'create', 'update', 'delete', 'close_period')
- `entity_type` TEXT (e.g., 'transaction', 'account', 'ledger', 'document')
- `entity_id` UUID
- `old_value` JSONB (state before the change, null on create)
- `new_value` JSONB (state after the change, null on delete)
- `created_at` TIMESTAMPTZ

The table MUST be indexed on `(ledger_id, created_at)` and
`(actor_id, created_at)` for efficient filtering.

#### Scenario: Transaction creation is logged

- **WHEN** a user creates a new transaction
- **THEN** an audit entry is created with `action='create'`,
  `entity_type='transaction'`, `old_value=null`, and
  `new_value` containing the transaction data.

#### Scenario: Account update is logged

- **WHEN** a user archives an account
- **THEN** an audit entry is created with `action='update'`,
  `entity_type='account'`, `old_value` containing
  `is_archived=false`, and `new_value` containing
  `is_archived=true`.

### Requirement: Audit Middleware

The system MUST record audit entries for all mutating operations
on core entities (accounts, transactions, documents, ledgers).
The audit MUST be recorded at the database level (via triggers)
or at the application level (via middleware/service layer) in a
way that cannot be bypassed by application code.

#### Scenario: All creates are audited

- **WHEN** any entity is created (account, transaction, document)
- **THEN** an audit entry with `action='create'` is written
  before the HTTP response is returned.

#### Scenario: All deletes are audited

- **WHEN** any entity is deleted
- **THEN** an audit entry with `action='delete'` is written
  with the `old_value` containing the deleted entity's data.

### Requirement: Audit Log UI

The system MUST provide a UI page to browse the audit log. The
page MUST support filtering by:

- Date range (from/to)
- Actor (user)
- Action (create/update/delete)
- Entity type (transaction/account/document/ledger)

The audit log MUST be displayed in reverse chronological order
(newest first) with pagination.

#### Scenario: User browses audit log

- **WHEN** a user navigates to the audit log page
- **THEN** they see a paginated list of audit entries with
  actor name, action, entity type, timestamp, and a summary
  of the change.

### Requirement: System-Level Events Logged

The following system-level events MUST also be recorded in the
audit log:

- User login / logout
- Period close
- Password change
- Role change (admin promoting/demoting users)

These events use `entity_type='system'` and do not have an
`entity_id`.

#### Scenario: Login is logged

- **WHEN** a user logs in
- **THEN** an audit entry with `action='login'`,
  `entity_type='system'`, `actor_id=<user_id>` is created.
