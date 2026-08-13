use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::budgets::{BudgetForm, BudgetList, BudgetRow, BudgetReport, BudgetVsActual},
    AppState,
};

#[derive(Deserialize)]
pub struct NewBudgetForm {
    pub account_id: String,
    pub period: String,
    pub amount: String,
    pub alert_threshold: Option<String>,
    pub start_date: String,
    pub end_date: String,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let budgets = sqlx::query_as::<_, BudgetRow>(
        r#"SELECT b.id, b.account_id, a.name AS account_name, b.period, b.amount,
                  b.alert_threshold, b.start_date, b.end_date
           FROM budgets b
           JOIN accounts a ON a.id = b.account_id
           WHERE b.ledger_id = $1
           ORDER BY b.start_date DESC, a.name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(BudgetList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        budgets,
        error: String::new(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let accounts = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT id, code, name FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(BudgetForm {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        accounts,
        period: "monthly".to_string(),
        amount: String::new(),
        alert_threshold: "0.8".to_string(),
        start_date: chrono::Utc::now().date_naive().to_string(),
        end_date: String::new(),
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewBudgetForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let account_id = Uuid::parse_str(&form.account_id)
        .map_err(|_| AppError::Validation("Invalid account".into()))?;
    let amount: Decimal = form
        .amount
        .parse()
        .map_err(|_| AppError::Validation("Invalid amount".into()))?;
    let alert_threshold: Decimal = form
        .alert_threshold
        .as_deref()
        .unwrap_or("0.8")
        .parse()
        .map_err(|_| AppError::Validation("Invalid threshold".into()))?;
    let period = form.period.clone();
    if !["monthly", "quarterly", "yearly"].contains(&period.as_str()) {
        return Err(AppError::Validation("Invalid period".into()));
    }
    let start_date = chrono::NaiveDate::parse_from_str(&form.start_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid start date".into()))?;
    let end_date = chrono::NaiveDate::parse_from_str(&form.end_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid end date".into()))?;
    if end_date < start_date {
        return Err(AppError::Validation("End date must be after start date".into()));
    }

    let budget_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO budgets (ledger_id, account_id, period, amount, alert_threshold, start_date, end_date)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .bind(&period)
    .bind(amount)
    .bind(alert_threshold)
    .bind(start_date)
    .bind(end_date)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "budget",
        Some(budget_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/budgets", ledger_id)).into_response())
}

pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, budget_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    sqlx::query("DELETE FROM budgets WHERE id = $1")
        .bind(budget_id)
        .execute(&state.pool)
        .await?;

    Ok(Redirect::to(&format!("/ledgers/{}/budgets", ledger_id)).into_response())
}

pub async fn report(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let from_str = params.get("from").cloned().unwrap_or_default();
    let to_str = params.get("to").cloned().unwrap_or_default();

    let today = chrono::Utc::now().date_naive();
    let from_date = if from_str.is_empty() {
        chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today)
    } else {
        chrono::NaiveDate::parse_from_str(&from_str, "%Y-%m-%d").unwrap_or(today)
    };
    let to_date = if to_str.is_empty() {
        today
    } else {
        chrono::NaiveDate::parse_from_str(&to_str, "%Y-%m-%d").unwrap_or(today)
    };

    let rows = sqlx::query_as::<_, (Uuid, String, Decimal, Decimal, chrono::NaiveDate, chrono::NaiveDate)>(
        r#"SELECT b.id, a.name, b.amount, b.alert_threshold, b.start_date, b.end_date
           FROM budgets b
           JOIN accounts a ON a.id = b.account_id
           WHERE b.ledger_id = $1
                 AND b.start_date <= $3
                 AND b.end_date >= $2"#,
    )
    .bind(ledger_id)
    .bind(from_date)
    .bind(to_date)
    .fetch_all(&state.pool)
    .await?;

    let mut report_rows: Vec<BudgetVsActual> = Vec::new();
    for (budget_id, account_name, budget_amount, _threshold, _start, _end) in rows {
        let actual: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(p.amount), 0) FROM postings p
               JOIN transactions t ON t.id = p.transaction_id
               WHERE p.account_id = (SELECT account_id FROM budgets WHERE id = $1)
                 AND t.ledger_id = $2
                 AND t.txn_date BETWEEN $3 AND $4
                 AND p.direction = 'DEBIT'"#,
        )
        .bind(budget_id)
        .bind(ledger_id)
        .bind(from_date)
        .bind(to_date)
        .fetch_one(&state.pool)
        .await?;

        let variance = budget_amount - actual;
        let variance_pct = if budget_amount > Decimal::ZERO {
            (variance / budget_amount) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        report_rows.push(BudgetVsActual {
            budget_id,
            account_name,
            budget_amount,
            actual,
            variance,
            variance_pct,
        });
    }

    Ok(render_response(BudgetReport {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        from: from_date.to_string(),
        to: to_date.to_string(),
        rows: report_rows,
    }))
}

pub async fn check_alerts(state: &AppState, user_id: Uuid) -> AppResult<()> {
    let budgets = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Decimal, Decimal, chrono::NaiveDate, chrono::NaiveDate)>(
        r#"SELECT id, ledger_id, account_id, amount, alert_threshold, start_date, end_date
           FROM budgets WHERE end_date >= CURRENT_DATE AND start_date <= CURRENT_DATE"#,
    )
    .fetch_all(&state.pool)
    .await?;

    for (budget_id, _ledger_id, account_id, amount, threshold, _start, _end) in budgets {
        let actual: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(p.amount), 0) FROM postings p
               JOIN transactions t ON t.id = p.transaction_id
               WHERE p.account_id = $1
                 AND t.txn_date BETWEEN CURRENT_DATE - INTERVAL '30 days' AND CURRENT_DATE
                 AND p.direction = 'DEBIT'"#,
        )
        .bind(account_id)
        .fetch_one(&state.pool)
        .await?;

        if amount > Decimal::ZERO && actual / amount >= threshold {
            let pct = actual / amount;
            let account_name: String = sqlx::query_scalar("SELECT name FROM accounts WHERE id = $1")
                .bind(account_id)
                .fetch_one(&state.pool)
                .await?;

            let message = format!(
                "{}: {:.1}% of budget used ({:.2} / {:.2})",
                account_name,
                pct * Decimal::from(100),
                actual,
                amount
            );

            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM budget_alerts WHERE budget_id = $1 AND acknowledged_at IS NULL)",
            )
            .bind(budget_id)
            .fetch_one(&state.pool)
            .await?;

            if !exists {
                sqlx::query(
                    r#"INSERT INTO budget_alerts (budget_id, user_id, message, threshold_pct, amount, budget_amount)
                       VALUES ($1, $2, $3, $4, $5, $6)"#,
                )
                .bind(budget_id)
                .bind(user_id)
                .bind(&message)
                .bind(pct)
                .bind(actual)
                .bind(amount)
                .execute(&state.pool)
                .await?;
            }
        }
    }

    Ok(())
}

pub async fn alerts(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let _ = check_alerts(&state, user.id).await;

    Ok(Redirect::to(&format!("/ledgers/{}/dashboard", ledger_id)).into_response())
}
