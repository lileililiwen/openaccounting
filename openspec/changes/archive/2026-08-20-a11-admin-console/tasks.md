# Admin Console

## 1. Testing

- [x] 1.1 HTTP: `admin_non_admin_blocked_403` — non-admin requests `/admin/audit` → 403.
- [x] 1.2 HTTP: `admin_suspend_user_blocks_login` — suspend → login rejected with generic error.
- [x] 1.3 HTTP: `admin_activate_user_restores_login`.
- [x] 1.4 HTTP: `admin_suspend_is_audited` — audit row with entity_type=user, old/new is_active.
- [x] 1.5 HTTP: `admin_promote_user` — role becomes admin + audit row.
- [x] 1.6 HTTP: `admin_demote_admin` — role becomes user + audit row.
- [x] 1.7 HTTP: `admin_self_suspend_422`.
- [x] 1.8 HTTP: `admin_last_admin_demote_422`.
- [x] 1.9 HTTP: `admin_audit_log_lists_all_ledgers` — entries from a ledger the admin does not belong to appear.
- [x] 1.10 HTTP: `admin_audit_log_filter_by_actor`.
- [x] 1.11 HTTP: `admin_audit_log_pagination` — page 2 offset.
- [x] 1.12 HTTP: `admin_user_detail_shows_ledgers_and_activity`.
- [x] 1.13 HTTP: `admin_dashboard_stats_and_feed`.

## 2. Implementation

- [x] 2.1 `src/handlers/admin.rs`: `POST /admin/users/{id}/status` (set `is_active`), with self-suspend guard.
- [x] 2.2 `src/handlers/admin.rs`: `POST /admin/users/{id}/role` (set role), with self + last-admin guards.
- [x] 2.3 `src/handlers/admin.rs`: `GET /admin/users/{id}` detail (profile, ledgers, recent activity).
- [x] 2.4 `src/handlers/admin.rs`: `GET /admin/audit` with filters (actor, action, entity_type, from, to) + pagination.
- [x] 2.5 Batch-resolve actor username + ledger name for audit rows (one query per page).
- [x] 2.6 `src/templates/admin.rs`: `AdminUsersDetailPage`, `AdminAuditPage` structs.
- [x] 2.7 `templates/admin/users.html`: per-row suspend/activate + promote/demote forms.
- [x] 2.8 `templates/admin/users_detail.html`: profile + ledgers + activity.
- [x] 2.9 `templates/admin/audit.html`: filter form + table + pager.
- [x] 2.10 Dashboard: extra stats (inactive users, documents, 24h activity), recent-activity feed, quick links.
- [x] 2.11 Routes wired into `admin_routes()` (all behind `require_admin`).

## 3. Validation

- [x] 3.1 `openspec validate a11-admin-console`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support --tests` clean for changed files.
- [x] 3.4 `cargo test --features test-support --test integration -- admin` passes.
- [x] 3.5 `openspec archive a11-admin-console`.
