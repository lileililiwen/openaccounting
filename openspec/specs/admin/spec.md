# admin Specification

## Purpose
TBD - created by archiving change add-admin-role-and-dashboard. Update Purpose after archive.
## Requirements
### Requirement: Role-Based Access Control

The system SHALL support a `role` column on the `users` table with
two possible values: `'user'` (default) and `'admin'`. The column
MUST be enforced at the database level via a `CHECK` constraint.

Every authenticated request to an admin-only route MUST verify the
user's role. Non-admin users MUST receive a `403 Forbidden` response.
Unauthenticated users MUST be redirected to login by the existing
`login_required!` layer (admin middleware runs after auth).

#### Scenario: Non-admin user is denied access

- **WHEN** a user with `role = 'user'` requests `GET /admin`
- **THEN** the response is `403 Forbidden` and the HTML body
  contains "403" or "Forbidden". No system data is leaked.

#### Scenario: Admin user is granted access

- **WHEN** a user with `role = 'admin'` requests `GET /admin`
- **THEN** the response is `200 OK` and the HTML body contains
  the admin dashboard with system stats.

#### Scenario: Unauthenticated user is redirected

- **WHEN** an unauthenticated client requests `GET /admin`
- **THEN** the response is `303 See Other` with `location: /login?next=/admin`.

### Requirement: Admin Dashboard

The system SHALL expose `GET /admin` to authenticated admin users.
The rendered page MUST display, at minimum:

- Total number of users in the system.
- Total number of ledgers across all users.
- Total number of transactions across all ledgers.
- The 10 most recently registered users (username, email,
  formatted `created_at`).

#### Scenario: Dashboard displays system stats

- **WHEN** an admin user requests `GET /admin`
- **THEN** the response is `200 OK` and the HTML body contains
  at least three stat cards (users, ledgers, transactions) and a
  table of recent users.

### Requirement: Admin User List

The system SHALL expose `GET /admin/users` to authenticated admin
users. The rendered page MUST display a table of all users with:

- `username`
- `email`
- `role` (displayed as a badge: "admin" or "user")
- `created_at` formatted as `YYYY-MM-DD`
- `is_active` status (active / inactive)

The list MUST be ordered by `created_at` descending (newest first).

#### Scenario: User list shows all users

- **WHEN** an admin user requests `GET /admin/users`
- **THEN** the response is `200 OK` and the HTML body contains a
  table with at least one row for every user in the system.

### Requirement: Admin Nav Link

The system navigation MUST show an "Admin" link in the top nav bar
when the authenticated user has `role = 'admin'`. The link MUST
point to `/admin`.

Regular users (`role = 'user'`) MUST NOT see the "Admin" link.

#### Scenario: Admin sees the link

- **WHEN** an admin user renders any authenticated page
- **THEN** the HTML contains `<a href="/admin">Admin</a>` in the
  nav bar.

#### Scenario: Regular user does not see the link

- **WHEN** a non-admin user renders any authenticated page
- **THEN** the HTML does NOT contain an `<a href="/admin">` link.

### Requirement: First User Promotion

When the migration runs on a database with existing users, the
user with the earliest `created_at` MUST be promoted to
`role = 'admin'`. If no users exist, the migration is a no-op.

This ensures the system always has at least one admin after the
migration, without requiring manual SQL.

#### Scenario: First user is promoted

- **WHEN** the migration runs on a database with one or more users
- **THEN** the user with the earliest `created_at` has
  `role = 'admin'` and all other users have `role = 'user'`.

#### Scenario: Empty database

- **WHEN** the migration runs on a database with zero users
- **THEN** no rows are updated. The first user to register will
  get `role = 'user'` (the default), and can be promoted manually.

