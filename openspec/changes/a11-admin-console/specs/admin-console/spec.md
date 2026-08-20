# admin-console Specification (delta)

## ADDED Requirements

### Requirement: Admin Only

MUST restrict every page and mutation in this change to users whose `role` is `admin`; non-admin users SHALL receive 403.

#### Scenario: Non-admin blocked

- **WHEN** a non-admin user requests `/admin/audit`
- **THEN** the response is 403.

### Requirement: Suspend / Activate User

MUST allow an admin to toggle a user's `is_active` flag from `/admin/users`; a suspended user MUST be refused at login with the generic login error; the toggle MUST be recorded in `audit_entries` with the admin as actor and old/new values.

#### Scenario: Suspend then login blocked

- **WHEN** an admin suspends a user and that user attempts to log in with correct credentials
- **THEN** login fails with the generic "Invalid email or password" error.

#### Scenario: Activate restores login

- **WHEN** an admin reactivates the user
- **THEN** the user can log in again.

#### Scenario: Toggle is audited

- **WHEN** an admin toggles a user's status
- **THEN** an `audit_entries` row is written with `entity_type = 'user'`, `entity_id` = the target user, and `new_value.is_active` reflecting the change.

### Requirement: Role Change

MUST allow an admin to promote a user to `admin` and to demote an `admin` to `user`; the change MUST be audit-logged.

#### Scenario: Promote

- **WHEN** an admin promotes a user
- **THEN** the user's `role` becomes `admin` and the audit log records it.

#### Scenario: Demote

- **WHEN** an admin demotes an admin
- **THEN** the target's `role` becomes `user` and the audit log records it.

### Requirement: Self-Protection

MUST refuse an admin action that would suspend or demote the admin's own account, and MUST refuse an action that would demote the last active admin; the response is 422 with a clear message.

#### Scenario: Self-suspend refused

- **WHEN** an admin attempts to suspend their own account
- **THEN** 422 and no change is made.

#### Scenario: Last admin demotion refused

- **WHEN** an admin attempts to demote the only remaining active admin
- **THEN** 422 and no change is made.

### Requirement: User Detail Page

MUST render `/admin/users/{id}` with the user's profile (username, email, role, status, joined), a list of their ledgers, and their most recent audit activity; the page MUST be admin-only.

#### Scenario: View user

- **WHEN** an admin opens a user's detail page
- **THEN** the profile, the user's ledgers, and their recent actions are shown.

### Requirement: System-Wide Audit Log

MUST render `/admin/audit` listing every `audit_entries` row across all ledgers, ordered newest-first, paginated at 50 rows; MUST support filtering by actor user, action, entity type, and date range; each row MUST show the actor username, ledger name (when set), action, entity type, timestamp, and a readable old→new summary when values exist.

#### Scenario: Unfiltered log

- **WHEN** an admin opens `/admin/audit`
- **THEN** the newest 50 entries across all users are shown with actor and ledger names.

#### Scenario: Filter by actor

- **WHEN** an admin filters by a specific user
- **THEN** only that user's entries are shown.

#### Scenario: Pagination

- **WHEN** more than 50 entries match
- **THEN** a pager appears and navigating to page 2 shows the next 50.

### Requirement: Dashboard Stats

MUST show on `/admin`, in addition to the existing user/ledger/transaction counters, the number of inactive users, the total document count, and the number of audit entries created in the last 24 hours.

#### Scenario: Stats render

- **WHEN** an admin opens the dashboard
- **THEN** the counters include inactive users, documents, and 24h activity.

### Requirement: Recent Activity Feed

MUST show the latest 10 `audit_entries` on `/admin` with actor, ledger, action, and timestamp; entries MUST link to the relevant user when resolvable.

#### Scenario: Feed on dashboard

- **WHEN** an admin opens the dashboard
- **THEN** the 10 most recent actions across all users are listed.

### Requirement: Dashboard Quick Links

SHALL link from `/admin` to `/admin/users`, `/admin/audit`, `/admin/backups`, and `/admin/integrity`.

#### Scenario: Management entry points

- **WHEN** an admin opens the dashboard
- **THEN** links to Users, Audit Log, Backups, and Integrity checks are visible.
