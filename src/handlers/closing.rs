use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

pub async fn close_year(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, year)): Path<(Uuid, i32)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut tx = state.pool.begin().await?;

    // Check if already closed
    let is_closed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM closed_periods WHERE ledger_id = $1 AND period_year = $2)",
    )
    .bind(ledger_id)
    .bind(year)
    .fetch_one(&mut *tx)
    .await?;
    if is_closed {
        return Err(AppError::Validation(format!(
            "Period {} is already closed.",
            year
        )));
    }

    // Get all income and expense accounts with their balances
    let period_start = NaiveDate::from_ymd_opt(year, 1, 1)
        .ok_or_else(|| AppError::Internal("Invalid year".into()))?;
    let period_end = NaiveDate::from_ymd_opt(year, 12, 31)
        .ok_or_else(|| AppError::Internal("Invalid year".into()))?;

    let account_balances = sqlx::query_as::<_, (Uuid, String, String, Decimal)>(
        r#"
        SELECT a.id, a.name, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw_net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date BETWEEN $2 AND $3
        WHERE a.ledger_id = $1
          AND a.type IN ('INCOME','EXPENSE')
        GROUP BY a.id, a.name, a.type
        HAVING COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) <> 0
        "#,
    )
    .bind(ledger_id)
    .bind(period_start)
    .bind(period_end)
    .fetch_all(&mut *tx)
    .await?;

    // Find or create the retained earnings account. The default chart of
    // accounts does not seed one, so period close must create it on the
    // fly rather than failing (`a17-reports-polish` surfaced this).
    let retained_earnings_id: Uuid = match sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND subtype = 'RETAINED_EARNINGS' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(id) => id,
        None => sqlx::query_scalar(
            "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
             VALUES ($1, 'Retained Earnings', '3050', 'EQUITY', 'RETAINED_EARNINGS', $2)
             RETURNING id",
        )
        .bind(ledger_id)
        .bind(&ledger.base_currency)
        .fetch_one(&mut *tx)
        .await?,
    };

    // Calculate net income
    let mut net_income = Decimal::ZERO;
    for (_, _, ty, raw) in &account_balances {
        match ty.as_str() {
            "INCOME" => net_income -= raw, // credits - debits (negative raw means credit balance)
            "EXPENSE" => net_income += raw, // debits - credits
            _ => {}
        }
    }

    // Create closing transaction
    let closing_date = period_end;
    let txn = sqlx::query_as::<_, crate::domain::Transaction>(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, currency, kind, created_by)
           VALUES ($1, $2, $3, $4, 'closing', $5)
           RETURNING id, ledger_id, txn_date, description, payee, reference, currency, kind,
                     contact_id, invoice_id, template_id, created_by, number, created_at, updated_at"#,
    )
    .bind(ledger_id)
    .bind(closing_date)
    .bind(format!("Year-end close {}", year))
    .bind(&ledger.base_currency)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    // Create closing postings for each income/expense account
    for (account_id, _name, ty, raw) in &account_balances {
        let amount = raw.abs();
        if amount == Decimal::ZERO {
            continue;
        }

        match ty.as_str() {
            "INCOME" => {
                // Income has credit balance (negative raw), so debit to close
                sqlx::query(
                    r#"INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
                       VALUES ($1, $2, $3, 'DEBIT', 'Close to retained earnings')"#,
                )
                .bind(txn.id)
                .bind(account_id)
                .bind(amount)
                .execute(&mut *tx)
                .await?;
            }
            "EXPENSE" => {
                // Expense has debit balance (positive raw), so credit to close
                sqlx::query(
                    r#"INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
                       VALUES ($1, $2, $3, 'CREDIT', 'Close to retained earnings')"#,
                )
                .bind(txn.id)
                .bind(account_id)
                .bind(amount)
                .execute(&mut *tx)
                .await?;
            }
            _ => {}
        }
    }

    // Create posting to retained earnings (balancing entry)
    if net_income > Decimal::ZERO {
        // Net income: credit retained earnings
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
               VALUES ($1, $2, $3, 'CREDIT', 'Net income for year')"#,
        )
        .bind(txn.id)
        .bind(retained_earnings_id)
        .bind(net_income)
        .execute(&mut *tx)
        .await?;
    } else if net_income < Decimal::ZERO {
        // Net loss: debit retained earnings
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
               VALUES ($1, $2, $3, 'DEBIT', 'Net loss for year')"#,
        )
        .bind(txn.id)
        .bind(retained_earnings_id)
        .bind(net_income.abs())
        .execute(&mut *tx)
        .await?;
    }

    // Mark period as closed
    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by) VALUES ($1, $2, $3)",
    )
    .bind(ledger_id)
    .bind(year)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Redirect::to(&format!("/ledgers/{}/reports", ledger_id)).into_response())
}
