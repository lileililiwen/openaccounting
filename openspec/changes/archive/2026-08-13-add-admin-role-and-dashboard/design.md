# Add admin role and dashboard — Design

## Context

The bootstrap schema has no concept of user roles. Every authenticated
user can perform every action. The account page change (archived)
added self-service password rotation but still no privilege
separation.

This change introduces a two-tier model: `user` (default) and
`admin`. Admins can view system-wide stats and a user list. All
other operations remain role-agnostic.

## Database

### Migration `0003_add_user_role.sql`

```sql
ALTER TABLE users
  ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
  CHECK (role IN ('user', 'admin'));

-- Promote the first registered user to admin.
UPDATE users
  SET role = 'admin'
WHERE id = (SELECT id FROM users ORDER BY created_at LIMIT 1);
```

The `DEFAULT 'user'` means all existing rows and future inserts
are safe without code changes.

### User struct update

```rust
// src/auth/mod.rs
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,          // ← NEW
    #[serde(skip_serializing)]
    pub hashed_password: String,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

Both `authenticate` and `get_user` queries are updated to SELECT
`role`. The `AuthUser::session_auth_hash()` remains
`hashed_password.as_bytes()` — role changes don't invalidate
sessions (they take effect on the next `get_user` call).

## Routes

```
GET   /admin              200   system dashboard (admin-only)
GET   /admin/users        200   paginated user list (admin-only)
```

Non-admin users hitting either route get a `403 Forbidden` page.

## Admin Middleware

```rust
// src/handlers/admin.rs
pub async fn require_admin(
    auth: AuthSession<Backend>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if user.role != "admin" {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}
```

The middleware is layered on a separate sub-router that sits inside
the existing `login_required!` layer. This means:
1. `login_required!` runs first (unauthenticated → 303).
2. `require_admin` runs second (non-admin → 403).

## Router Wiring (main.rs)

```rust
let admin_routes = Router::new()
    .route("/admin", get(handlers::admin::dashboard))
    .route("/admin/users", get(handlers::admin::users))
    .route_layer(axum::middleware::from_fn(require_admin));

// The protected router already has login_required!.
let protected = Router::new()
    // ... existing routes ...
    .merge(admin_routes)
    .route_layer(login_required!(Backend));
```

## Templates

### `templates/admin/dashboard.html`

```
Admin Dashboard
├── Stats cards
│   ├── Total users        (count)
│   ├── Total ledgers      (count)
│   └── Total transactions (count)
└── Recent activity
    └── last 10 users (username, email, joined_at)
```

### `templates/admin/users.html`

```
Admin — Users
├── Table
│   ├── Username
│   ├── Email
│   ├── Role              (badge: admin / user)
│   ├── Joined            (formatted date)
│   └── Status            (active / inactive)
└── Pagination (prev/next)
```

Both templates extend `base.html` and include `partials/_nav.html`.

## Nav Update

`partials/_nav.html` gains a conditional admin link:

```html
{% if user.role == "admin" %}
  <a href="/admin" class="px-2 py-1 rounded text-slate-600 hover:bg-slate-100">Admin</a>
{% endif %}
```

This requires the nav partial to have access to `user.role`. Since
the partial is included in all templates, we pass `user` (the
`User` struct) as a top-level field to every template. The existing
templates already do this (or will need a minor update to pass
`user`).

Actually — looking at the nav partial, it already has access to
`username` (a `String`). We need to add `user_role: String` to
the nav context. All template structs will need a
`pub user_role: String` field. This is the same pattern already
established with `username` / `ledger_id` / `ledger_name`.

## Tests

### Unit (`src/handlers/admin.rs::tests`)

- `require_admin_returns_403_for_non_admin_user`
- `require_admin_passes_for_admin_user`
- `require_admin_returns_401_for_unauthenticated`

### Integration (manual)

- Register a new user → role is `user` → `/admin` returns 403.
- Manually `UPDATE users SET role = 'admin' WHERE id = ...` →
  `/admin` returns 200 and shows stats.

## Risks / Trade-offs

- **R1: Session role is stale until next `get_user` call.**
  → Acceptable. Every request already calls `get_user` via
  `axum-login`, so the role is always fresh within one request.
- **R2: First-user promotion is a one-shot migration.**
  → Acceptable for v0.1. If no users exist at migration time,
  the UPDATE is a no-op. Admin can be set manually via SQL.
- **R3: No audit log of admin actions.**
  → Explicitly out of scope for v0.1.
- **R4: The `role` column is TEXT, not an enum type.**
  → Acceptable. Postgres CHECK constraint enforces validity. We
  can migrate to a real ENUM later if needed.

## Migration Plan

Non-destructive. The `ALTER TABLE` adds a column with a safe
default. The `UPDATE` promotes the first user. No data is lost.
Rollback: `ALTER TABLE users DROP COLUMN role;`.

## Open Questions

- **Q1:** Should we add a `promote_user` / `demote_user` admin
  endpoint? → Defer to v0.2; for now, manual SQL.
- **Q2:** Should we add an admin-only user-detail page
  (`/admin/users/{id}`)? → Defer; the list is sufficient for now.
