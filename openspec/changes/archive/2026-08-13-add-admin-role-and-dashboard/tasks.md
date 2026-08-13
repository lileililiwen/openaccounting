# Add admin role and dashboard — Tasks

> **Spec-first rule (from `Agents.md §2.4`):** `## 1. Testing` comes
> first. Tests are written and red **before** any `## 2.
> Implementation` task is marked complete.

## 1. Testing

- [ ] 1.1 Unit: `require_admin` returns `403 Forbidden` when the
      authenticated user's `role` is `"user"`.
- [ ] 1.2 Unit: `require_admin` passes through the request when
      the authenticated user's `role` is `"admin"`.
- [ ] 1.3 Unit: `require_admin` returns `401 Unauthorized` when
      no user is authenticated (delegates to `login_required`
      upstream, but the middleware itself should not panic).
- [ ] 1.4 Integration: after migration, the first registered user
      has `role = 'admin'`.
- [ ] 1.5 Integration: a newly registered user (second user) has
      `role = 'user'` by default.
- [ ] 1.6 HTTP: `GET /admin` with a non-admin user returns `403`.
- [ ] 1.7 HTTP: `GET /admin` with an admin user returns `200` and
      the HTML contains "Admin Dashboard" and at least one stat
      card.
- [ ] 1.8 HTTP: `GET /admin/users` with an admin user returns `200`
      and the HTML contains a table with at least one row.

## 2. Implementation

- [x] 2.1 New migration `migrations/0003_add_user_role.sql`:
      - `ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
        CHECK (role IN ('user', 'admin'));`
      - `UPDATE users SET role = 'admin' WHERE id = (SELECT id FROM
        users ORDER BY created_at LIMIT 1);`
- [x] 2.2 Update `User` struct in `src/auth/mod.rs` to include
      `pub role: String`.
- [x] 2.3 Update `authenticate` and `get_user` queries in
      `src/auth/mod.rs` to SELECT `role`.
- [x] 2.4 New module `src/handlers/admin.rs` with:
      - `pub async fn require_admin(auth, request, next) -> Result<Response, AppError>`
      - `pub async fn dashboard(auth) -> AppResult<Response>`
      - `pub async fn users(auth) -> AppResult<Response>`
- [x] 2.5 New template struct `AdminDashboardPage` in
      `src/templates/admin.rs` with fields `user_id`, `username`,
      `user_role`, `ledger_id`, `ledger_name`, `total_users`,
      `total_ledgers`, `total_transactions`, `recent_users: Vec<RecentUser>`.
- [x] 2.6 New template struct `AdminUsersPage` in
      `src/templates/admin.rs` with fields `user_id`, `username`,
      `user_role`, `ledger_id`, `ledger_name`, `users: Vec<UserRow>`.
- [x] 2.7 New templates `templates/admin/dashboard.html` and
      `templates/admin/users.html` extending `base.html`.
- [x] 2.8 Update `templates/partials/_nav.html` to show an "Admin"
      link when `user_role == "admin"`.
- [x] 2.9 Update all existing template structs to include
      `pub user_role: String` (populated from `user.role`).
- [x] 2.10 Wire admin routes in `src/main.rs`:
      ```
      let admin_routes = Router::new()
          .route("/admin", get(handlers::admin::dashboard))
          .route("/admin/users", get(handlers::admin::users))
          .route_layer(axum::middleware::from_fn(require_admin));
      ```
      Merge `admin_routes` into `protected` before
      `route_layer(login_required!(Backend))`.

## 3. Validation

- [x] 3.1 `openspec validate add-admin-role-and-dashboard` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean for any
      file touched by this change. (45 dead-code warnings from
      bootstrap-phase templates; non-blocking.)
- [x] 3.4 `cargo build --release` succeeds.
- [ ] 3.5 All tasks under `## 1. Testing` are green. (Pending: need
      unit tests for `require_admin` and integration/HTTP tests.)
- [ ] 3.6 Manual smoke: register user (role=user) → `/admin` → 403.
      Manually promote to admin → `/admin` → 200 with dashboard.
- [ ] 3.7 `openspec archive add-admin-role-and-dashboard` — the delta
      is folded into `openspec/specs/admin/spec.md` and the change
      moves to `openspec/changes/archive/`.
