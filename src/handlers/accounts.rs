use crate::templates::render_response;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{
    extract::{Path, State},
    Form,
};
use axum_login::AuthSession;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    auth::Backend,
    domain::{Account, AccountType},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::accounts::{AccountGroup, AccountList, AccountNew},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // Current balance per account.
    let balances = compute_balances(&state, ledger_id).await?;

    // Group accounts by type for the chart-of-accounts view.
    let groups: Vec<AccountGroup> = vec![
        ("ASSET", AccountType::Asset),
        ("LIABILITY", AccountType::Liability),
        ("EQUITY", AccountType::Equity),
        ("INCOME", AccountType::Income),
        ("EXPENSE", AccountType::Expense),
    ]
    .into_iter()
    .map(|(label, ty)| AccountGroup {
        account_type: label.to_string(),
        accounts: accounts
            .iter()
            .filter(|a| a.r#type == ty.as_str())
            .cloned()
            .collect(),
    })
    .filter(|g| !g.accounts.is_empty())
    .collect();

    Ok(render_response(AccountList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        groups,
        balances,
    }))
}

pub async fn balances_for(state: &AppState, ledger_id: Uuid) -> AppResult<HashMap<Uuid, Decimal>> {
    compute_balances(state, ledger_id).await
}

async fn compute_balances(state: &AppState, ledger_id: Uuid) -> AppResult<HashMap<Uuid, Decimal>> {
    let rows = sqlx::query_as::<_, (Uuid, String, Decimal)>(
        r#"
        SELECT a.id, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        WHERE a.ledger_id = $1
        GROUP BY a.id, a.type
        "#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let mut map = HashMap::new();
    for (id, ty, raw) in rows {
        let val = match ty.as_str() {
            "ASSET" | "EXPENSE" => raw,
            _ => -raw,
        };
        map.insert(id, val);
    }
    Ok(map)
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    Ok(render_response(AccountNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        account_types: vec![
            AccountType::Asset,
            AccountType::Liability,
            AccountType::Equity,
            AccountType::Income,
            AccountType::Expense,
        ],
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewAccountForm {
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub description: Option<String>,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewAccountForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let account_type = match form.account_type.as_str() {
        "ASSET" => AccountType::Asset,
        "LIABILITY" => AccountType::Liability,
        "EQUITY" => AccountType::Equity,
        "INCOME" => AccountType::Income,
        "EXPENSE" => AccountType::Expense,
        other => {
            return Ok(render_response(AccountNew {
                user_id: user.id,
                username: user.username.clone(),
                user_role: user.role.clone(),
                ledger_id,
                ledger_name: ledger.name.clone(),
                account_types: vec![],
                error: format!("Invalid account type: {}", other),
            }));
        }
    };
    let name = form.name.trim();
    if name.is_empty() {
        return Ok(render_response(AccountNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name.clone(),
            account_types: vec![],
            error: "Name is required".into(),
        }));
    }
    sqlx::query(
        r#"INSERT INTO accounts (ledger_id, name, code, type, currency, description)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(
        form.code
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(account_type.as_str())
    .bind(&ledger.base_currency)
    .bind(
        form.description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .execute(&state.pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Account name already exists in this ledger".into())
        }
        _ => AppError::Db(e),
    })?;

    Ok(Redirect::to(&format!("/ledgers/{}/accounts", ledger_id)).into_response())
}
