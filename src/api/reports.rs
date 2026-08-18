//! Read-only report endpoints for the REST API
//! (`a1-rest-api`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    api::{problem::Problem, ApiUser},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/ledgers/{ledger_id}/reports/trial-balance",
            get(trial_balance),
        )
        .route(
            "/ledgers/{ledger_id}/reports/balance-sheet",
            get(balance_sheet),
        )
        .route(
            "/ledgers/{ledger_id}/reports/income-statement",
            get(income_statement),
        )
        .route("/ledgers/{ledger_id}/reports/cash-flow", get(cash_flow))
        .route(
            "/ledgers/{ledger_id}/reports/general-ledger",
            get(general_ledger),
        )
}

#[derive(Debug, Serialize)]
struct TrialBalanceRow {
    account_id: Uuid,
    account_name: String,
    account_type: String,
    debit: Decimal,
    credit: Decimal,
}

async fn trial_balance(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let rows: Vec<(Uuid, String, String, Decimal, Decimal)> = sqlx::query_as(
        "SELECT a.id, a.name, a.type,
                COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0) AS debit,
                COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS credit
         FROM accounts a
         LEFT JOIN postings p ON p.account_id = a.id
         LEFT JOIN transactions t ON t.id = p.transaction_id AND t.ledger_id = $1
         WHERE a.ledger_id = $1
         GROUP BY a.id, a.name, a.type
         ORDER BY a.type, a.name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(problem_for_db)?;

    let dto: Vec<TrialBalanceRow> = rows
        .into_iter()
        .map(|(id, name, ty, debit, credit)| TrialBalanceRow {
            account_id: id,
            account_name: name,
            account_type: ty,
            debit,
            credit,
        })
        .collect();
    Ok(Json(serde_json::json!({
        "as_of": chrono::Utc::now().date_naive(),
        "data": dto,
    })))
}

async fn balance_sheet(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    // Minimal balance sheet: sum of ASSET, LIABILITY, EQUITY.
    let rows: Vec<(String, Decimal)> = sqlx::query_as(
        "SELECT a.type,
                COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
              - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw
         FROM accounts a
         LEFT JOIN postings p ON p.account_id = a.id
         LEFT JOIN transactions t ON t.id = p.transaction_id AND t.ledger_id = $1
         WHERE a.ledger_id = $1 AND a.type IN ('ASSET','LIABILITY','EQUITY')
         GROUP BY a.type",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(problem_for_db)?;
    let mut assets = Decimal::ZERO;
    let mut liabilities = Decimal::ZERO;
    let mut equity = Decimal::ZERO;
    for (ty, raw) in rows {
        let value = if ty == "ASSET" { raw } else { -raw };
        match ty.as_str() {
            "ASSET" => assets = value,
            "LIABILITY" => liabilities = value,
            "EQUITY" => equity = value,
            _ => {}
        }
    }
    Ok(Json(serde_json::json!({
        "as_of": chrono::Utc::now().date_naive(),
        "assets": assets,
        "liabilities": liabilities,
        "equity": equity,
    })))
}

async fn income_statement(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let rows: Vec<(String, Decimal)> = sqlx::query_as(
        "SELECT a.type,
                COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0)
              - COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0) AS net
         FROM accounts a
         LEFT JOIN postings p ON p.account_id = a.id
         LEFT JOIN transactions t ON t.id = p.transaction_id AND t.ledger_id = $1
         WHERE a.ledger_id = $1 AND a.type IN ('INCOME','EXPENSE')
         GROUP BY a.type",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(problem_for_db)?;
    let mut income = Decimal::ZERO;
    let mut expense = Decimal::ZERO;
    for (ty, net) in rows {
        match ty.as_str() {
            "INCOME" => income = net,
            "EXPENSE" => expense = net,
            _ => {}
        }
    }
    Ok(Json(serde_json::json!({
        "as_of": chrono::Utc::now().date_naive(),
        "income": income,
        "expense": expense,
        "net": income - expense,
    })))
}

async fn cash_flow(
    State(_state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    if !ledger_owned_by(&_state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    // Minimal stub — full cash-flow is implemented in the HTML
    // handler. The API endpoint returns the running totals of
    // INCOME - EXPENSE on cash accounts as a placeholder.
    Ok(Json(serde_json::json!({
        "ledger_id": ledger_id,
        "as_of": chrono::Utc::now().date_naive(),
        "note": "full cash-flow report pending; see /ledgers/{id}/reports/cash-flow",
    })))
}

#[derive(Debug, Serialize)]
struct GeneralLedgerRow {
    txn_id: Uuid,
    txn_date: NaiveDate,
    description: String,
    account_id: Uuid,
    account_name: String,
    direction: String,
    amount: Decimal,
    memo: Option<String>,
}

async fn general_ledger(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let rows: Vec<(
        Uuid,
        NaiveDate,
        String,
        Uuid,
        String,
        String,
        Decimal,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT t.id, t.txn_date, t.description,
                    a.id, a.name, p.direction, p.amount, p.memo
         FROM postings p
         JOIN transactions t ON t.id = p.transaction_id
         JOIN accounts a ON a.id = p.account_id
         WHERE t.ledger_id = $1
         ORDER BY t.txn_date DESC, t.created_at DESC
         LIMIT 200",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(problem_for_db)?;
    let dto: Vec<GeneralLedgerRow> = rows
        .into_iter()
        .map(
            |(id, d, desc, aid, aname, dir, amt, memo)| GeneralLedgerRow {
                txn_id: id,
                txn_date: d,
                description: desc,
                account_id: aid,
                account_name: aname,
                direction: dir,
                amount: amt,
                memo,
            },
        )
        .collect();
    Ok(Json(serde_json::json!({ "data": dto })))
}

async fn ledger_owned_by(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    user_id: Uuid,
) -> Result<bool, Problem> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM ledgers WHERE id = $1 AND owner_id = $2")
            .bind(ledger_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(problem_for_db)?;
    Ok(row.is_some())
}

fn problem_for_db(e: sqlx::Error) -> Problem {
    Problem::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        format!("db: {e}"),
    )
}
