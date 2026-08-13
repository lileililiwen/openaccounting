use crate::templates::render_response;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{
    extract::{Path, State},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    domain::Ledger,
    error::{AppError, AppResult},
    templates::ledgers::{LedgerList, LedgerNew, LedgerShow},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledgers = sqlx::query_as::<_, (Uuid, String, String, String, String)>(
        r#"SELECT l.id, l.name, l.base_currency, 'owner' AS role, l.created_at::text
           FROM ledgers l
           WHERE l.owner_id = $1
           UNION ALL
           SELECT l.id, l.name, l.base_currency, lm.role, l.created_at::text
           FROM ledgers l
           JOIN ledger_members lm ON lm.ledger_id = l.id
           WHERE lm.user_id = $1
           ORDER BY created_at DESC"#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let shares: Vec<(Uuid, String, String, String)> = ledgers
        .into_iter()
        .map(|(id, name, currency, role, _ts)| (id, name, currency, role))
        .collect();

    let pending_invitations: Vec<(Uuid, String, String)> = sqlx::query_as(
        r#"SELECT li.id, l.name, li.role
           FROM ledger_invitations li
           JOIN ledgers l ON l.id = li.ledger_id
           WHERE li.invitee_email = $1 AND li.status = 'pending'"#,
    )
    .bind(&user.email)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(LedgerList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        ledgers: shares,
        flash: String::new(),
        pending_invitations,
    }))
}

pub async fn new_page(auth: AuthSession<Backend>) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    Ok(render_response(LedgerNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewLedgerForm {
    pub name: String,
    pub base_currency: String,
    pub timezone: String,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<NewLedgerForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let name = form.name.trim();
    if name.is_empty() {
        return Ok(render_response(LedgerNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            error: "Name is required".into(),
        }));
    }
    let currency = form.base_currency.trim().to_uppercase();
    if currency.len() != 3 {
        return Ok(render_response(LedgerNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            error: "Currency must be a 3-letter code (e.g. USD, EUR)".into(),
        }));
    }
    let timezone = if form.timezone.trim().is_empty() {
        "UTC".to_string()
    } else {
        form.timezone.clone()
    };

    let mut tx = state.pool.begin().await?;
    let ledger = sqlx::query_as::<_, Ledger>(
        r#"INSERT INTO ledgers (owner_id, name, base_currency, timezone)
           VALUES ($1, $2, $3, $4)
           RETURNING id, owner_id, name, base_currency, timezone, created_at, updated_at"#,
    )
    .bind(user.id)
    .bind(name)
    .bind(&currency)
    .bind(&timezone)
    .fetch_one(&mut *tx)
    .await?;

    // Seed default chart of accounts.
    for acct in Ledger::default_chart_of_accounts(&currency) {
        sqlx::query(
            r#"INSERT INTO accounts (ledger_id, name, code, type, subtype, currency, is_archived)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(ledger.id)
        .bind(&acct.name)
        .bind(&acct.code)
        .bind(acct.account_type.as_str())
        .bind(acct.account_subtype.as_str())
        .bind(&acct.currency)
        .bind(acct.is_archived)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // Audit log
    let _ = audit::log(
        &state.pool,
        Some(ledger.id),
        user.id,
        "create",
        "ledger",
        Some(ledger.id),
        None,
        Some(serde_json::json!({
            "name": name,
            "currency": currency
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}", ledger.id)).into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = sqlx::query_as::<_, Ledger>(
        "SELECT id, owner_id, name, base_currency, timezone, created_at, updated_at
         FROM ledgers WHERE id = $1 AND owner_id = $2",
    )
    .bind(ledger_id)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let (account_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&state.pool)
            .await?;
    let (txn_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&state.pool)
            .await?;

    Ok(render_response(LedgerShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: ledger.id,
        ledger_name: ledger.name.clone(),
        ledger,
        account_count,
        txn_count,
    }))
}

/// Middleware-style helper used by other handlers: ensures the current user
/// owns the given ledger.
pub async fn ensure_owner(state: &AppState, user_id: Uuid, ledger_id: Uuid) -> AppResult<Ledger> {
    let ledger = sqlx::query_as::<_, Ledger>(
        "SELECT id, owner_id, name, base_currency, timezone, created_at, updated_at
         FROM ledgers WHERE id = $1 AND owner_id = $2",
    )
    .bind(ledger_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(ledger)
}

/// Ensures the user can access the ledger (owner, editor, or viewer).
/// Returns the ledger and the user's role: "owner", "editor", or "viewer".
pub async fn ensure_access(
    state: &AppState,
    user_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<(Ledger, String)> {
    let ledger = sqlx::query_as::<_, Ledger>(
        "SELECT id, owner_id, name, base_currency, timezone, created_at, updated_at
         FROM ledgers WHERE id = $1",
    )
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    if ledger.owner_id == user_id {
        return Ok((ledger, "owner".to_string()));
    }

    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM ledger_members WHERE ledger_id = $1 AND user_id = $2",
    )
    .bind(ledger_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    match role {
        Some(r) => Ok((ledger, r)),
        None => Err(AppError::NotFound),
    }
}

/// Ensures the user can edit (owner or editor).
pub async fn ensure_editor(
    state: &AppState,
    user_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<(Ledger, String)> {
    let (ledger, role) = ensure_access(state, user_id, ledger_id).await?;
    if role == "viewer" {
        return Err(AppError::Unauthorized);
    }
    Ok((ledger, role))
}
