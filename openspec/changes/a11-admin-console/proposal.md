# Admin Console

## Why

The admin area today is read-only: `/admin` shows three counters and
a recent-users list, `/admin/users` is a table with no actions. A
self-hosted bookkeeping tool needs an operator who can **audit what
users do** and **manage access** — suspend an account, change roles,
and review the activity trail across every ledger. The underlying
infrastructure already exists (`audit_entries` rows are written for
every mutating action and `is_active` is enforced at login); it is
simply not exposed to admins.

## What Changes

- `/admin/users` gains per-row actions: suspend / activate, and
  promote / demote role.
- New user detail page `/admin/users/{id}` showing profile, the
  user's ledgers, and their recent activity.
- New system-wide audit log `/admin/audit`: every `audit_entries`
  row across all users and ledgers, filterable by actor / action /
  entity type / date range, paginated.
- The `/admin` dashboard gains additional stats, a recent-activity
  feed, and quick links to the management pages.
- Every admin action (suspend, role change) is itself audit-logged.

## Capabilities

### New Capabilities

- `admin-user-management`: suspend/activate and role changes with
  self/last-admin guard rails.
- `admin-audit-log`: system-wide activity trail with filters.
- `admin-dashboard`: stats, activity feed, quick links.

## Impact

**New files:**

- `templates/admin/users_detail.html`.
- `templates/admin/audit.html`.

**Modified files:**

- `src/handlers/admin.rs` — new handlers + routes.
- `src/templates/admin.rs` — new page structs.
- `templates/admin/dashboard.html`, `templates/admin/users.html`.
- `tests/integration/admin.rs` — new HTTP tests.
