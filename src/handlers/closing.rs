use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::close_controls,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::render_response,
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
        None => {
            sqlx::query_scalar(
                "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
             VALUES ($1, 'Retained Earnings', '3050', 'EQUITY', 'RETAINED_EARNINGS', $2)
             RETURNING id",
            )
            .bind(ledger_id)
            .bind(&ledger.base_currency)
            .fetch_one(&mut *tx)
            .await?
        }
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
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by, closed_through) VALUES ($1, $2, $3, $4)",
    )
    .bind(ledger_id)
    .bind(year)
    .bind(user.id)
    .bind(period_end)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "close",
        "period",
        None,
        None,
        Some(serde_json::json!({ "year": year, "closed_through": period_end })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/reports", ledger_id)).into_response())
}

#[derive(Deserialize)]
pub struct ClosePeriodForm {
    pub closed_through: String,
}

/// Hard-close the ledger through a date (`pro-close-controls`).
/// Owner or global admin only. Writes `closed_periods` + audit row.
pub async fn close_period(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<ClosePeriodForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_or_admin(&state, user.id, ledger_id).await?;

    let through = NaiveDate::parse_from_str(form.closed_through.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid closed_through date (YYYY-MM-DD)".into()))?;
    let year = through.format("%Y").to_string().parse::<i32>().unwrap_or(0);

    let existing = close_controls::closed_through_for(&state.pool, ledger_id).await?;
    if let Some(w) = existing {
        if through <= w {
            return Err(AppError::Validation(format!(
                "Ledger is already closed through {w}."
            )));
        }
    }

    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by, closed_through)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(ledger_id)
    .bind(year)
    .bind(user.id)
    .bind(through)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "close",
        "period",
        None,
        None,
        Some(serde_json::json!({ "closed_through": through })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/close", ledger_id)).into_response())
}

#[derive(Deserialize)]
pub struct ReopenForm {
    pub reason: String,
    pub closed_through: Option<String>,
}

/// Reopen a closed period with mandatory reason (min 10 chars).
/// Owner or global admin only. Writes `reopen_events` + audit row
/// and removes watermarks at/after the given date (or all when
/// no date is supplied, i.e. full reopen).
pub async fn reopen_period(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<ReopenForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_or_admin(&state, user.id, ledger_id).await?;

    let reason = close_controls::validate_reason(&form.reason).map_err(AppError::Validation)?;

    let through: Option<NaiveDate> = match form.closed_through.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(s) => Some(
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| AppError::Validation("Invalid closed_through date".into()))?,
        ),
    };

    if through.is_some() {
        sqlx::query("DELETE FROM closed_periods WHERE ledger_id = $1 AND closed_through >= $2")
            .bind(ledger_id)
            .bind(through)
            .execute(&state.pool)
            .await?;
        // Legacy year rows covering the same range.
        if let Some(d) = through {
            let y = d.format("%Y").to_string().parse::<i32>().unwrap_or(0);
            sqlx::query(
                "DELETE FROM closed_periods WHERE ledger_id = $1 AND period_year >= $2 AND closed_through IS NULL",
            )
            .bind(ledger_id)
            .bind(y)
            .execute(&state.pool)
            .await?;
        }
    } else {
        sqlx::query("DELETE FROM closed_periods WHERE ledger_id = $1")
            .bind(ledger_id)
            .execute(&state.pool)
            .await?;
    }

    sqlx::query(
        "INSERT INTO reopen_events (ledger_id, closed_through, reopened_by, reason)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(ledger_id)
    .bind(through)
    .bind(user.id)
    .bind(&reason)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reopen",
        "period",
        None,
        None,
        Some(serde_json::json!({ "closed_through": through, "reason": reason })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/close", ledger_id)).into_response())
}

/// Close / reopen status page with banner + forms.
pub async fn close_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let (ledger, _) = ledgers::ensure_access(&state, user.id, ledger_id).await?;
    let watermark = close_controls::closed_through_for(&state.pool, ledger_id).await?;
    let reopens: Vec<(NaiveDate, String, Option<NaiveDate>)> = sqlx::query_as(
        "SELECT created_at::DATE, reason, closed_through FROM reopen_events
         WHERE ledger_id = $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    Ok(render_response(crate::templates::closing::ClosePage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "reports".to_string(),
        closed_through: watermark,
        reopens: reopens
            .into_iter()
            .map(|(d, r, t)| (d.to_string(), r, t.map(|x| x.to_string())))
            .collect(),
    }))
}

/// Pending-approval queue page.
pub async fn approval_queue(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let pending: Vec<(Uuid, String, NaiveDate, Decimal, Uuid)> = sqlx::query_as(
        r#"SELECT t.id, t.description, t.txn_date, ja.amount, ja.maker
           FROM journal_approvals ja JOIN transactions t ON t.id = ja.txn_id
           WHERE ja.ledger_id = $1 AND ja.status = 'pending' ORDER BY ja.created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(crate::templates::closing::ApprovalQueue {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        pending: pending
            .into_iter()
            .map(|(id, d, dt, a, m)| (id, d, dt.to_string(), a.to_string(), m))
            .collect(),
    }))
}

/// Approve a pending journal. Maker != checker enforced in service (403).
pub async fn approve(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    match crate::domain::posting_service::PostingService::approve_journal(
        &state.pool,
        ledger_id,
        txn_id,
        user.id,
    )
    .await
    {
        Ok(()) => Ok(Redirect::to(&format!("/ledgers/{}/approvals", ledger_id)).into_response()),
        Err(crate::domain::posting_service::PostingServiceError::SelfApproval) => Err(
            AppError::ForbiddenMsg("Maker cannot approve their own journal.".into()),
        ),
        Err(crate::domain::posting_service::PostingServiceError::NotPending) => Err(
            AppError::Validation("Journal is not pending approval.".into()),
        ),
        Err(crate::domain::posting_service::PostingServiceError::Db(e)) => Err(AppError::Db(e)),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

/// Reject a pending journal (same separation-of-duties rule).
pub async fn reject(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    match crate::domain::posting_service::PostingService::reject_journal(
        &state.pool,
        ledger_id,
        txn_id,
        user.id,
    )
    .await
    {
        Ok(()) => Ok(Redirect::to(&format!("/ledgers/{}/approvals", ledger_id)).into_response()),
        Err(crate::domain::posting_service::PostingServiceError::SelfApproval) => Err(
            AppError::ForbiddenMsg("Maker cannot reject their own journal.".into()),
        ),
        Err(crate::domain::posting_service::PostingServiceError::NotPending) => Err(
            AppError::Validation("Journal is not pending approval.".into()),
        ),
        Err(crate::domain::posting_service::PostingServiceError::Db(e)) => Err(AppError::Db(e)),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct ThresholdForm {
    pub approval_threshold: String,
}

/// Update the maker-checker threshold. Owner or global admin only.
pub async fn update_threshold(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<ThresholdForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_or_admin(&state, user.id, ledger_id).await?;
    let threshold: Decimal = form
        .approval_threshold
        .trim()
        .parse()
        .map_err(|_| AppError::Validation("Invalid threshold".into()))?;
    if threshold < Decimal::ZERO {
        return Err(AppError::Validation("Threshold cannot be negative".into()));
    }
    sqlx::query("UPDATE ledgers SET approval_threshold = $2 WHERE id = $1")
        .bind(ledger_id)
        .bind(threshold)
        .execute(&state.pool)
        .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "update_threshold",
        "ledger",
        Some(ledger_id),
        None,
        Some(serde_json::json!({ "approval_threshold": threshold })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{}/close", ledger_id)).into_response())
}
