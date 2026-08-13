# Add admin role and dashboard

## Why

There is no way to distinguish regular users from administrators. Every
user has the same privileges. For an accounting system this is a
compliance risk: certain operations (viewing all users, auditing
account changes, rotating credentials) should be restricted to a small
set of trusted users. Without an admin role, any user can register
and immediately access the full system.

This change adds a `role` column to `users` (`'user'` or `'admin'`),
an admin-only middleware, and an admin dashboard showing system-wide
user stats.

## What Changes

- **Migration `0003_add_user_role.sql`**: adds `role TEXT NOT NULL
  DEFAULT 'user' CHECK (role IN ('user', 'admin'))` to `users`.
  The very first registered user is promoted to `'admin'` in the
  same migration (using `UPDATE users SET role = 'admin' WHERE id =
  (SELECT id FROM users ORDER BY created_at LIMIT 1)`).
- **`User` struct** gains a `pub role: String` field (loaded from
  the DB). The `AuthnBackend::authenticate` and `get_user` queries
  are updated to SELECT `role`.
- **Admin middleware** `require_admin`: extracts the user from the
  session, returns 403 if `user.role != 'admin'`.
- **New handler module `src/handlers/admin.rs`**:
  - `GET /admin` — system dashboard (total users, total ledgers,
    total transactions, recent activity).
  - `GET /admin/users` — paginated user list with roles.
- **New templates**: `templates/admin/dashboard.html`,
  `templates/admin/users.html`.
- **Nav update**: if `user.role == 'admin'`, show an "Admin" link
  in the top nav pointing to `/admin`.
- **Routes**: new `/admin` + `/admin/users` routes inside a
  `require_admin` sub-router, layered on top of the existing
  `login_required!` layer.

## Capabilities

### New Capabilities

- `admin` — administrator-only features: system dashboard and user
  management. New spec: `specs/admin/spec.md`.

### Modified Capabilities

- `architecture` — updated to document the admin role column and
  middleware in the schema section.

## Impact

- **New files:** `migrations/0003_add_user_role.sql`,
  `src/handlers/admin.rs`, `templates/admin/dashboard.html`,
  `templates/admin/users.html`, `openspec/specs/admin/spec.md`.
- **Modified files:** `src/main.rs` (admin routes + middleware),
  `src/auth/mod.rs` (`User` struct gets `role` field, queries
  updated), `templates/partials/_nav.html` (admin link).
- **Database:** additive column (`role`) with safe default. No data
  loss. First user auto-promoted.
- **Sessions:** existing sessions are **not** invalidated by the
  schema change. The `role` field is loaded fresh on each
  `get_user` call, so existing sessions gain admin status only
  after the next `get_user` round-trip (which happens on every
  request).

## Non-Goals (v0.1)

- Admin UI for promoting / demoting users (manual SQL for now).
- Audit logging of admin actions.
- Role-based access control beyond the two-tier user/admin model.
- Admin API keys or programmatic access.
- Email notifications for role changes.
