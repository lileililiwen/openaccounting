use axum::{
    extract::{Form, Path, State},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    Router,
};
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    templates::{
        admin::{AdminDashboardPage, AdminUsersPage, RecentUserRow, UserRow},
        render_response,
    },
    AppState,
};

/// Middleware: require the authenticated user to have `role = 'admin'`.
pub async fn require_admin(
    auth: AuthSession<Backend>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if user.role != "admin" {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}

/// Admin-only router, layered with `require_admin`.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin", axum::routing::get(dashboard))
        .route("/admin/users", axum::routing::get(users))
        .route("/admin/users/{id}", axum::routing::get(user_detail))
        .route(
            "/admin/users/{id}/status",
            axum::routing::post(set_user_status),
        )
        .route("/admin/users/{id}/role", axum::routing::post(set_user_role))
        .route(
            "/admin/ocr-corpus.json",
            axum::routing::get(crate::handlers::document_ocr_feedback::export_corpus),
        )
        .route("/admin/audit", axum::routing::get(audit_log))
        .route(
            "/admin/audit/verify",
            axum::routing::get(crate::handlers::admin_audit_verify::verify_chain),
        )
        .route_layer(middleware::from_fn(require_admin))
}

pub async fn dashboard(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<impl axum::response::IntoResponse> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let total_users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;

    let total_ledgers: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ledgers")
        .fetch_one(&state.pool)
        .await?;

    let total_transactions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(&state.pool)
        .await?;

    #[derive(sqlx::FromRow)]
    struct RecentUser {
        username: String,
        email: String,
        role: String,
        created_at: time::OffsetDateTime,
    }

    let recent_users = sqlx::query_as::<_, RecentUser>(
        "SELECT username, email, role, created_at FROM users ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(&state.pool)
    .await?;

    let page = AdminDashboardPage::new(
        user.clone(),
        total_users.0,
        total_ledgers.0,
        total_transactions.0,
        recent_users
            .into_iter()
            .map(|u| RecentUserRow {
                username: u.username,
                email: u.email,
                role: u.role,
                created_at_display: crate::templates::account::fmt_date(&u.created_at),
            })
            .collect(),
    );

    Ok(render_response(page))
}

pub async fn users(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<impl axum::response::IntoResponse> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    #[derive(sqlx::FromRow)]
    struct UserRowDb {
        id: Uuid,
        username: String,
        email: String,
        role: String,
        is_active: bool,
        created_at: time::OffsetDateTime,
    }

    let rows = sqlx::query_as::<_, UserRowDb>(
        "SELECT id, username, email, role, is_active, created_at FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let page = AdminUsersPage::new(
        user.clone(),
        rows.into_iter()
            .map(|r| UserRow {
                id: r.id,
                username: r.username,
                email: r.email,
                role: r.role,
                is_active: r.is_active,
                is_self: r.id == user.id,
                created_at_display: crate::templates::account::fmt_date(&r.created_at),
            })
            .collect(),
    );

    Ok(render_response(page))
}

/// Form body for `POST /admin/users/{id}/status` — the target's
/// desired `is_active` value.
#[derive(Deserialize)]
pub struct UserStatusForm {
    pub is_active: bool,
}

/// Suspend or activate a user (`a11-admin-console`). Suspension is
/// enforced at login (`src/auth/mod.rs`); the toggle is audit-logged.
pub async fn set_user_status(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Form(form): Form<UserStatusForm>,
) -> AppResult<Redirect> {
    let admin = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    // Guard rail: an admin must not be able to suspend their own
    // account (`a11-admin-console` Self-Protection).
    if user_id == admin.id && !form.is_active {
        return Err(AppError::Unprocessable(
            "You cannot suspend your own account.".into(),
        ));
    }

    let current: Option<(String, bool)> =
        sqlx::query_as("SELECT email, is_active FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((email, was_active)) = current else {
        return Err(AppError::NotFound);
    };
    let _ = email;
    if was_active == form.is_active {
        // No-op: redirect without writing an audit row.
        return Ok(Redirect::to("/admin/users"));
    }

    sqlx::query("UPDATE users SET is_active = $1 WHERE id = $2")
        .bind(form.is_active)
        .bind(user_id)
        .execute(&state.pool)
        .await?;

    audit::log(
        &state.pool,
        None,
        admin.id,
        "set_status",
        "user",
        Some(user_id),
        Some(serde_json::json!({ "is_active": was_active })),
        Some(serde_json::json!({ "is_active": form.is_active })),
    )
    .await?;

    Ok(Redirect::to("/admin/users"))
}

/// Form body for `POST /admin/users/{id}/role` — the target's
/// desired role (`admin` or `user`).
#[derive(Deserialize)]
pub struct UserRoleForm {
    pub role: String,
}

/// Promote or demote a user (`a11-admin-console`). The audit log
/// records the change.
pub async fn set_user_role(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Form(form): Form<UserRoleForm>,
) -> AppResult<Redirect> {
    let admin = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    if form.role != "admin" && form.role != "user" {
        return Err(AppError::Unprocessable(format!(
            "Invalid role: {}",
            form.role
        )));
    }
    // Guard rail: an admin must not change their own role
    // (`a11-admin-console` Self-Protection).
    if user_id == admin.id {
        return Err(AppError::Unprocessable(
            "You cannot change your own role.".into(),
        ));
    }

    let current: Option<(String,)> = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some((old_role,)) = current else {
        return Err(AppError::NotFound);
    };
    if old_role == form.role {
        return Ok(Redirect::to("/admin/users"));
    }

    // Guard rail: never leave the system without an active admin
    // (`a11-admin-console` Self-Protection). Refuse any admin
    // demotion while exactly one active admin remains (the acting
    // admin themselves), since it could drop the count to zero.
    if form.role != "admin" && old_role == "admin" {
        let active_admins: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE")
                .fetch_one(&state.pool)
                .await?;
        if active_admins.0 <= 1 {
            return Err(AppError::Unprocessable(
                "Cannot demote the last active admin.".into(),
            ));
        }
    }

    sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
        .bind(&form.role)
        .bind(user_id)
        .execute(&state.pool)
        .await?;

    audit::log(
        &state.pool,
        None,
        admin.id,
        "set_role",
        "user",
        Some(user_id),
        Some(serde_json::json!({ "role": old_role })),
        Some(serde_json::json!({ "role": form.role })),
    )
    .await?;

    Ok(Redirect::to("/admin/users"))
}

/// Render a readable `old → new` summary for an audit entry's
/// JSONB values (e.g. `role: user → admin`).
fn value_summary(old: Option<&serde_json::Value>, new: Option<&serde_json::Value>) -> String {
    let flatten = |v: Option<&serde_json::Value>| -> Vec<(String, String)> {
        match v {
            Some(serde_json::Value::Object(map)) => map
                .iter()
                .map(|(k, val)| (k.clone(), val.to_string()))
                .collect(),
            Some(v) => vec![("value".to_string(), v.to_string())],
            None => vec![],
        }
    };
    let o = flatten(old);
    let n = flatten(new);
    let mut out: Vec<String> = Vec::new();
    for (k, ov) in &o {
        match n.iter().find(|(nk, _)| nk == k) {
            Some((_, nv)) => out.push(format!("{k}: {ov} → {nv}")),
            None => out.push(format!("{k}: {ov} → —")),
        }
    }
    for (k, nv) in &n {
        if !o.iter().any(|(ok, _)| ok == k) {
            out.push(format!("{k}: — → {nv}"));
        }
    }
    out.join(", ")
}

/// Render `/admin/users/{id}` — a user's profile, their ledgers,
/// and their recent activity (`a11-admin-console` User Detail Page).
pub async fn user_detail(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<impl axum::response::IntoResponse> {
    let admin = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    #[derive(sqlx::FromRow)]
    struct UserDb {
        username: String,
        email: String,
        role: String,
        is_active: bool,
        created_at: time::OffsetDateTime,
    }

    let user_row = sqlx::query_as::<_, UserDb>(
        "SELECT username, email, role, is_active, created_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    #[derive(sqlx::FromRow)]
    struct LedgerRow {
        id: Uuid,
        name: String,
        base_currency: String,
        created_at: time::OffsetDateTime,
    }

    let ledgers = sqlx::query_as::<_, LedgerRow>(
        "SELECT id, name, base_currency, created_at FROM ledgers
         WHERE owner_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    // The user's most recent audit activity across every ledger.
    let activity = crate::audit::list(
        &state.pool,
        None,
        Some(user_id),
        None,
        None,
        None,
        None,
        10,
        0,
    )
    .await?;

    // Batch-resolve ledger names for the activity rows.
    let ledger_ids: Vec<Uuid> = activity.iter().filter_map(|e| e.ledger_id).collect();
    let ledger_names: std::collections::HashMap<Uuid, String> = if ledger_ids.is_empty() {
        Default::default()
    } else {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM ledgers WHERE id = ANY($1)")
            .bind(&ledger_ids)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .collect()
    };

    let page = crate::templates::admin::AdminUserDetailPage::new(
        admin.clone(),
        crate::templates::admin::UserDetailHeader {
            username: user_row.username,
            email: user_row.email,
            role: user_row.role,
            is_active: user_row.is_active,
            joined_display: crate::templates::account::fmt_date(&user_row.created_at),
        },
        ledgers
            .into_iter()
            .map(|l| crate::templates::admin::UserLedgerRow {
                id: l.id,
                name: l.name,
                base_currency: l.base_currency,
                created_display: crate::templates::account::fmt_date(&l.created_at),
            })
            .collect(),
        activity
            .into_iter()
            .map(|e| crate::templates::admin::AuditActivityRow {
                action: e.action,
                entity_type: e.entity_type,
                entity_id: e.entity_id.unwrap_or_default().to_string(),
                ledger_name: e
                    .ledger_id
                    .and_then(|id| ledger_names.get(&id).cloned())
                    .unwrap_or_default(),
                summary: value_summary(e.old_value.as_ref(), e.new_value.as_ref()),
                created_display: e.created_at.format("%Y-%m-%d %H:%M").to_string(),
            })
            .collect(),
    );

    Ok(render_response(page))
}

/// Query params for `/admin/audit` (`a11-admin-console` System-Wide
/// Audit Log).
#[derive(Deserialize)]
pub struct AuditLogQuery {
    pub actor: Option<Uuid>,
    pub action: Option<String>,
    pub entity: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<i64>,
}

fn parse_date_filter(s: &str, end_of_day: bool) -> Option<chrono::DateTime<chrono::Utc>> {
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    if end_of_day {
        Some(d.and_hms_opt(23, 59, 59)?.and_utc())
    } else {
        Some(d.and_hms_opt(0, 0, 0)?.and_utc())
    }
}

/// Render `/admin/audit` — every audit entry across all users and
/// ledgers, newest first, filterable and paginated.
pub async fn audit_log(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<AuditLogQuery>,
) -> AppResult<impl axum::response::IntoResponse> {
    let admin = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    const PAGE_SIZE: i64 = 50;
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let from = q.from.as_deref().and_then(|s| parse_date_filter(s, false));
    let to = q.to.as_deref().and_then(|s| parse_date_filter(s, true));

    // Fetch one extra row to detect "has more" for the pager.
    let entries = crate::audit::list(
        &state.pool,
        None,
        q.actor,
        q.action.as_deref(),
        q.entity.as_deref(),
        from,
        to,
        PAGE_SIZE + 1,
        offset,
    )
    .await?;
    let has_more = entries.len() as i64 > PAGE_SIZE;
    let entries: Vec<_> = entries.into_iter().take(PAGE_SIZE as usize).collect();

    // Batch-resolve actor usernames and ledger names for this page.
    let actor_ids: Vec<Uuid> = entries.iter().map(|e| e.actor_id).collect();
    let ledger_ids: Vec<Uuid> = entries.iter().filter_map(|e| e.ledger_id).collect();
    let actor_names: std::collections::HashMap<Uuid, String> = if actor_ids.is_empty() {
        Default::default()
    } else {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, username FROM users WHERE id = ANY($1)")
            .bind(&actor_ids)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .collect()
    };
    let ledger_names: std::collections::HashMap<Uuid, String> = if ledger_ids.is_empty() {
        Default::default()
    } else {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM ledgers WHERE id = ANY($1)")
            .bind(&ledger_ids)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .collect()
    };

    // User dropdown for the actor filter.
    let filter_users = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, username, email FROM users ORDER BY username",
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(
        |(id, username, email)| crate::templates::admin::AuditFilterUser {
            id,
            username,
            email,
        },
    )
    .collect::<Vec<_>>();

    let page_model = crate::templates::admin::AdminAuditPage::new(
        admin.clone(),
        filter_users,
        crate::templates::admin::AuditFilters {
            actor_id: q.actor.map(|v| v.to_string()).unwrap_or_default(),
            action: q.action.unwrap_or_default(),
            entity: q.entity.unwrap_or_default(),
            from: q.from.unwrap_or_default(),
            to: q.to.unwrap_or_default(),
        },
        entries
            .into_iter()
            .map(|e| crate::templates::admin::AuditLogRow {
                created_display: e.created_at.format("%Y-%m-%d %H:%M").to_string(),
                actor_username: actor_names.get(&e.actor_id).cloned().unwrap_or_default(),
                ledger_name: e
                    .ledger_id
                    .and_then(|id| ledger_names.get(&id).cloned())
                    .unwrap_or_default(),
                action: e.action,
                entity_type: e.entity_type,
                entity_id: e.entity_id.unwrap_or_default().to_string(),
                summary: value_summary(e.old_value.as_ref(), e.new_value.as_ref()),
            })
            .collect(),
        page,
        has_more,
    );

    Ok(render_response(page_model))
}

#[cfg(test)]
mod tests {
    use crate::auth::{password::hash_password, User};

    async fn setup() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .ok()?;
        Some(pool)
    }

    async fn create_test_user(pool: &sqlx::PgPool, tag: &str, role: &str) -> User {
        let email = format!("{tag}@example.com");
        let username = tag.to_string();
        let hashed = hash_password("password123").unwrap();
        sqlx::query_as::<_, User>(
            r#"INSERT INTO users (email, username, hashed_password, display_name, role)
               VALUES ($1, $2, $3, $1, $4)
               ON CONFLICT (email) DO UPDATE SET role = $4
               RETURNING id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale"#,
        )
        .bind(&email)
        .bind(&username)
        .bind(&hashed)
        .bind(role)
        .fetch_one(pool)
        .await
        .expect("create test user")
    }

    async fn cleanup(pool: &sqlx::PgPool, email: &str) {
        let _ = sqlx::query("DELETE FROM users WHERE email = $1")
            .bind(email)
            .execute(pool)
            .await;
    }

    #[tokio::test]
    async fn admin_role_is_stored_and_retrieved() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "admin-role-check", "admin").await;
        assert_eq!(user.role, "admin");
        cleanup(&pool, &user.email).await;
    }

    #[tokio::test]
    async fn user_role_is_stored_and_retrieved() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "user-role-check", "user").await;
        assert_ne!(user.role, "admin");
        assert_eq!(user.role, "user");
        cleanup(&pool, &user.email).await;
    }

    #[tokio::test]
    async fn non_admin_role_fails_admin_check() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "non-admin-check", "user").await;
        let is_admin = user.role == "admin";
        assert!(!is_admin, "non-admin user should fail admin check");
        cleanup(&pool, &user.email).await;
    }

    #[tokio::test]
    async fn admin_role_passes_admin_check() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "admin-pass-check", "admin").await;
        let is_admin = user.role == "admin";
        assert!(is_admin, "admin user should pass admin check");
        cleanup(&pool, &user.email).await;
    }
}
