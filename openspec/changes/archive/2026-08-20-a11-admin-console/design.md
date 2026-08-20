# Admin Console

## Context

OpenAccounting is self-hosted; whoever runs the server is the
operator. The operator currently cannot see what any user has done
across ledgers, nor manage accounts. The audit trail (`audit_entries`)
is written on every mutating action but only surfaced per-ledger to
the ledger's own members. `is_active` already gates login, but there
is no UI to flip it.

## Goals / Non-Goals

**Goals:**

- Give admins a read view of every user's activity (audit log).
- Let admins suspend/activate users and change roles, with guard
  rails so the operator cannot lock themselves (or the last admin)
  out.
- Make the dashboard a real entry point to the management pages.

**Non-Goals:**

- Editing or deleting another user's ledger data directly — the
  per-ledger permissions and append-only modes remain the data
  integrity model; admins audit, they do not rewrite books.
- Multi-tenant/enterprise features (RBAC groups, SSO).

## Decisions

- **Reuse `audit::list`**: it already supports actor/action/entity/
  date filters and LIMIT/OFFSET pagination. The admin log calls it
  with no ledger filter and adds a side query to resolve actor
  username + ledger name for display.
- **User writes go through `audit::log`**: suspending or changing a
  role records `actor_id` = the admin, `entity_type = "user"`,
  `entity_id` = the target user, and old/new values — so the action
  itself is auditable.
- **`is_active` is the only enforcement point**: login already
  rejects `!user.is_active` (`src/auth/mod.rs:82`). No session
  invalidation for suspended users in v1; the next login is blocked.
- **Guard rails are handler-level**: an admin MUST NOT be able to
  suspend or demote themselves, and MUST NOT be able to demote the
  last active admin. Enforced in the handler, not the DB.

## Risks / Trade-offs

- Sessions of a suspended user stay valid until they log out; the
  suspension only blocks the next login. **Mitigation:** acceptable
  for a self-hosted tool; document it in the UI text.
- Resolving actor/ledger names with a side query is N+1-prone on the
  audit page. **Mitigation:** batch the lookup with one `IN (...)` per
  page of rows (≤ 50 rows), not per-row queries.
