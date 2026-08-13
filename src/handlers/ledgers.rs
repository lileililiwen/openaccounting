use crate::templates::render_response;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{
    extract::{Path, State},
    Form,
};
use axum_login::AuthSession;
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
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
    let ledgers = sqlx::query_as::<_, Ledger>(
        "SELECT id, owner_id, name, base_currency, timezone, created_at, updated_at
         FROM ledgers WHERE owner_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(LedgerList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        ledgers,
        flash: String::new(),
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
