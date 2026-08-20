# Admin Console

## 1. Testing

- [ ] 1.1 HTTP: `admin_non_admin_blocked_403` — non-admin requests `/admin/audit` → 403.
- [ ] 1.2 HTTP: `admin_suspend_user_blocks_login` — suspend → login rejected with generic error.
- [ ] 1.3 HTTP: `admin_activate_user_restores_login`.
- [ ] 1.4 HTTP: `admin_suspend_is_audited` — audit row with entity_type=user, old/new is_active.
- [ ] 1.5 HTTP: `admin_promote_user` — role becomes admin + audit row.
- [ ] 1.6 HTTP: `admin_demote_admin` — role becomes user + audit row.
- [ ] 1.7 HTTP: `admin_self_suspend_422`.
- [ ] 1.8 HTTP: `admin_last_admin_demote_422`.
- [ ] 1.9 HTTP: `admin_audit_log_lists_all_ledgers` — entries from a ledger the admin does not belong to appear.
- [ ] 1.10 HTTP: `admin_audit_log_filter_by_actor`.
- [ ] 1.11 HTTP: `admin_audit_log_pagination` — page 2 offset.
- [ ] 1.12 HTTP: `admin_user_detail_shows_ledgers_and_activity`.
- [ ] 1.13 HTTP: `admin_dashboard_stats_and_feed`.

## 2. Implementation

- [ ] 2.1 `src/handlers/admin.rs`: `POST /admin/users/{id}/status` (set `is_active`), with self-suspend guard.
- [ ] 2.2 `src/handlers/admin.rs`: `POST /admin/users/{id}/role` (set role), with self + last-admin guards.
- [ ] 2.3 `src/handlers/admin.rs`: `GET /admin/users/{id}` detail (profile, ledgers, recent activity).
- [ ] 2.4 `src/handlers/admin.rs`: `GET /admin/audit` with filters (actor, action, entity_type, from, to) + pagination.
- [ ] 2.5 Batch-resolve actor username + ledger name for audit rows (one query per page).
- [ ] 2.6 `src/templates/admin.rs`: `AdminUsersDetailPage`, `AdminAuditPage` structs.
- [ ] 2.7 `templates/admin/users.html`: per-row suspend/activate + promote/demote forms.
- [ ] 2.8 `templates/admin/users_detail.html`: profile + ledgers + activity.
- [ ] 2.9 `templates/admin/audit.html`: filter form + table + pager.
- [ ] 2.10 Dashboard: extra stats (inactive users, documents, 24h activity), recent-activity feed, quick links.
- [ ] 2.11 Routes wired into `admin_routes()` (all behind `require_admin`).

## 3. Validation

- [ ] 3.1 `openspec validate a11-admin-console`.
- [ ] 3.2 `cargo fmt --check`.
- [ ] 3.3 `cargo clippy --features test-support --tests` clean for changed files.
- [ ] 3.4 `cargo test --features test-support --test integration -- admin` passes.
- [ ] 3.5 `openspec archive a11-admin-console`.
