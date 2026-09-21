//! Statement reconciliation sessions (`statement-reconciliation`).
//!
//! Session lifecycle per bank/cash account:
//! open (opening carried from the prior closed session) → clear lines
//! (explicit confirm each) → finish (zero-difference gate, locks) →
//! unreconcile-with-reason (audit-logged, lines return to uncleared).
//! Rule suggestions surface as cleared *candidates* — auto-clear always
//! requires explicit user confirm.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum::Json;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::{reconciliation_rules, statement_reconciliation as domain},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::rec_session::{RecSessionLine, RecSessionPage},
    AppState,
};

#[derive(Deserialize)]
pub struct CreateSessionForm {
    pub stmt_close_date: NaiveDate,
    pub stmt_close_balance: Decimal,
}

#[derive(Deserialize)]
pub struct ClearForm {
    pub bank_line_id: Uuid,
    /// Explicit user confirm (`statement-reconciliation` 2.4): auto-clear
    /// and manual clear both require `confirm` to be truthy.
    pub confirm: Option<String>,
}

#[derive(Deserialize)]
pub struct UnclearForm {
    pub bank_line_id: Uuid,
}

#[derive(Deserialize)]
pub struct UnreconcileForm {
    pub reason: Option<String>,
}

fn confirm_given(v: &Option<String>) -> bool {
    matches!(
        v.as_deref().map(str::trim),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

struct Session {
    id: Uuid,
    stmt_close_date: NaiveDate,
    stmt_close_balance: Decimal,
    opening_balance: Decimal,
    status: String,
}

async fn load_session(
    state: &AppState,
    ledger_id: Uuid,
    account_id: Uuid,
    session_id: Uuid,
) -> AppResult<Session> {
    let row: Option<(NaiveDate, Decimal, Decimal, String)> = sqlx::query_as(
        r#"SELECT stmt_close_date, stmt_close_balance, opening_balance, status
           FROM rec_sessions WHERE id = $1 AND ledger_id = $2 AND account_id = $3"#,
    )
    .bind(session_id)
    .bind(ledger_id)
    .bind(account_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((stmt_close_date, stmt_close_balance, opening_balance, status)) = row else {
        return Err(AppError::NotFound);
    };
    Ok(Session {
        id: session_id,
        stmt_close_date,
        stmt_close_balance,
        opening_balance,
        status,
    })
}

async fn cleared_sum(state: &AppState, session_id: Uuid) -> AppResult<Decimal> {
    let sum: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(b.amount), 0)
           FROM rec_lines r JOIN bank_statement_lines b ON b.id = r.bank_line_id
           WHERE r.session_id = $1 AND r.cleared"#,
    )
    .bind(session_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(sum)
}

/// POST /ledgers/{id}/reconcile/{account_id}/sessions — open a session.
/// Opening balance carries forward from the latest closed session.
pub async fn create_session(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<CreateSessionForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let prior: Option<Decimal> = sqlx::query_scalar(
        r#"SELECT stmt_close_balance FROM rec_sessions
           WHERE ledger_id = $1 AND account_id = $2 AND status = 'closed'
           ORDER BY stmt_close_date DESC LIMIT 1"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    let opening = domain::carry_opening(prior);

    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO rec_sessions
           (ledger_id, account_id, stmt_close_date, stmt_close_balance, opening_balance, status, created_by)
           VALUES ($1, $2, $3, $4, $5, 'open', $6) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .bind(form.stmt_close_date)
    .bind(form.stmt_close_balance)
    .bind(opening)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        // Duplicate period for the account surfaces as a conflict,
        // not a 500 (`statement-reconciliation` session uniqueness).
        if let sqlx::Error::Database(db) = &e {
            if db.code().as_deref() == Some("23505") {
                return AppError::Conflict(
                    "a session already exists for this account and close date".into(),
                );
            }
        }
        AppError::Db(e)
    })?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reconcile_session_open",
        "rec_session",
        Some(id),
        None,
        Some(serde_json::json!({
            "account_id": account_id.to_string(),
            "stmt_close_date": form.stmt_close_date.to_string(),
            "stmt_close_balance": form.stmt_close_balance.to_string(),
            "opening_balance": opening.to_string(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reconcile/{account_id}/sessions/{id}"
    ))
    .into_response())
}

fn suggest_rule(
    rules: &[reconciliation_rules::Rule],
    description: &str,
    amount: Decimal,
    currency: &str,
) -> Option<String> {
    let cents = (amount * Decimal::new(100, 0)).round();
    let amount_cents: i64 = cents.try_into().ok()?;
    let line = reconciliation_rules::Line {
        description: description.to_string(),
        amount_cents,
        currency: currency.to_string(),
    };
    reconciliation_rules::first_match(rules, &line).map(|(rule, _)| rule.name.clone())
}

async fn active_rules(
    state: &AppState,
    ledger_id: Uuid,
) -> AppResult<Vec<reconciliation_rules::Rule>> {
    let rows: Vec<(
        Uuid,
        String,
        String,
        i32,
        serde_json::Value,
        serde_json::Value,
    )> = sqlx::query_as(
        r#"SELECT id, name, kind, priority, predicate, action
               FROM reconciliation_rules WHERE ledger_id = $1 AND is_active ORDER BY priority ASC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, name, kind, priority, predicate, action)| {
            Some(reconciliation_rules::Rule {
                id,
                name,
                kind: reconciliation_rules::RuleKind::parse(&kind)?,
                priority,
                predicate,
                action,
                is_active: true,
            })
        })
        .collect())
}

/// GET session page: difference indicator, lock banner, rule suggestions
/// rendered as cleared candidates with confirm checkboxes.
pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let session = load_session(&state, ledger_id, account_id, session_id).await?;

    let account_currency: String =
        sqlx::query_scalar("SELECT currency FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten()
            .unwrap_or_default();

    // Uncleared-first, paginated (large statements must not blow the
    // page; `idx_bsl_account` + `idx_rec_lines_session` cover this).
    let rows: Vec<(Uuid, NaiveDate, String, Decimal, bool)> = sqlx::query_as(
        r#"SELECT b.id, b.statement_date, COALESCE(b.description, ''),
                  b.amount, COALESCE(r.cleared, FALSE)
           FROM bank_statement_lines b
           LEFT JOIN rec_lines r ON r.bank_line_id = b.id AND r.session_id = $3
           WHERE b.ledger_id = $1 AND b.account_id = $2
           ORDER BY COALESCE(r.cleared, FALSE) ASC, b.statement_date DESC
           LIMIT 500"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    let cleared: Decimal = cleared_sum(&state, session_id).await?;
    let difference =
        domain::compute_difference(session.opening_balance, cleared, session.stmt_close_balance);

    // Rule suggestions surface as candidates only — clearing still
    // requires the confirm checkbox per line.
    let rules = active_rules(&state, ledger_id).await.unwrap_or_default();
    let lines: Vec<RecSessionLine> = rows
        .into_iter()
        .map(|(id, statement_date, description, amount, cleared_flag)| {
            let suggested_rule = if cleared_flag {
                None
            } else {
                suggest_rule(&rules, &description, amount, &account_currency)
            };
            RecSessionLine {
                id,
                statement_date,
                description,
                amount,
                cleared: cleared_flag,
                suggested_rule,
            }
        })
        .collect();

    Ok(crate::templates::render_response(RecSessionPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        account_id,
        session_id: session.id,
        stmt_close_date: session.stmt_close_date,
        stmt_close_balance: session.stmt_close_balance,
        opening_balance: session.opening_balance,
        cleared_sum: cleared,
        difference,
        status: session.status.clone(),
        locked: session.status == "closed",
        lines,
        flash: String::new(),
    }))
}

/// GET suggestions JSON: rule-matched cleared candidates for a session.
/// Clearing any candidate still requires POST clear with confirm.
pub async fn suggestions(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let _session = load_session(&state, ledger_id, account_id, session_id).await?;

    let account_currency: String =
        sqlx::query_scalar("SELECT currency FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten()
            .unwrap_or_default();

    let rows: Vec<(Uuid, String, Decimal)> = sqlx::query_as(
        r#"SELECT b.id, COALESCE(b.description, ''), b.amount
           FROM bank_statement_lines b
           LEFT JOIN rec_lines r ON r.bank_line_id = b.id AND r.session_id = $3
           WHERE b.ledger_id = $1 AND b.account_id = $2 AND COALESCE(r.cleared, FALSE) = FALSE"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    let rules = active_rules(&state, ledger_id).await?;
    let candidates: Vec<serde_json::Value> = rows
        .iter()
        .filter_map(|(id, description, amount)| {
            suggest_rule(&rules, description, *amount, &account_currency).map(|rule| {
                serde_json::json!({
                    "bank_line_id": id,
                    "description": description,
                    "amount": amount.to_string(),
                    "rule": rule,
                })
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "candidates": candidates })).into_response())
}

/// POST clear: mark one statement line cleared inside an open session.
/// Requires explicit `confirm`; closed sessions reject with 409 (lock).
pub async fn clear(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
    Form(form): Form<ClearForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let session = load_session(&state, ledger_id, account_id, session_id).await?;

    if session.status == "closed" {
        return Err(AppError::Conflict(
            "session is closed and locked; unreconcile with a reason to change lines".into(),
        ));
    }
    if !confirm_given(&form.confirm) {
        return Err(AppError::Validation(
            "clearing requires explicit confirm (pass confirm=1)".into(),
        ));
    }

    let belongs: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM bank_statement_lines WHERE id = $1 AND ledger_id = $2 AND account_id = $3",
    )
    .bind(form.bank_line_id)
    .bind(ledger_id)
    .bind(account_id)
    .fetch_optional(&state.pool)
    .await?;
    if belongs.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        r#"INSERT INTO rec_lines (session_id, bank_line_id, cleared, cleared_at)
           VALUES ($1, $2, TRUE, now())
           ON CONFLICT (session_id, bank_line_id)
           DO UPDATE SET cleared = TRUE, cleared_at = now()"#,
    )
    .bind(session_id)
    .bind(form.bank_line_id)
    .execute(&state.pool)
    .await?;

    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reconcile/{account_id}/sessions/{session_id}"
    ))
    .into_response())
}

/// POST unclear: reversible only while the session is open.
/// Closed sessions reject with 409 (lock).
pub async fn unclear(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
    Form(form): Form<UnclearForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let session = load_session(&state, ledger_id, account_id, session_id).await?;

    if session.status == "closed" {
        return Err(AppError::Conflict(
            "session is closed and locked; unreconcile with a reason to change lines".into(),
        ));
    }

    sqlx::query(
        "UPDATE rec_lines SET cleared = FALSE, cleared_at = NULL WHERE session_id = $1 AND bank_line_id = $2",
    )
    .bind(session_id)
    .bind(form.bank_line_id)
    .execute(&state.pool)
    .await?;

    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reconcile/{account_id}/sessions/{session_id}"
    ))
    .into_response())
}

/// POST finish: zero-difference gate. Non-zero leaves the session open
/// with 409 showing the difference; zero closes and locks.
pub async fn finish(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let session = load_session(&state, ledger_id, account_id, session_id).await?;

    if session.status == "closed" {
        return Err(AppError::Conflict("session is already closed".into()));
    }

    let cleared = cleared_sum(&state, session_id).await?;
    let difference =
        domain::compute_difference(session.opening_balance, cleared, session.stmt_close_balance);
    if let Err(d) = domain::validate_finish(difference) {
        return Err(AppError::Conflict(format!(
            "difference {d} must be zero to finish; clear the remaining lines first"
        )));
    }

    sqlx::query(
        "UPDATE rec_sessions SET status = 'closed', closed_at = now(), closed_by = $2 WHERE id = $1",
    )
    .bind(session_id)
    .bind(user.id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reconcile_session_finish",
        "rec_session",
        Some(session_id),
        None,
        Some(serde_json::json!({
            "account_id": account_id.to_string(),
            "stmt_close_balance": session.stmt_close_balance.to_string(),
            "opening_balance": session.opening_balance.to_string(),
            "cleared_sum": cleared.to_string(),
        })),
    )
    .await;

    crate::observability::metrics::reconciliation_completed();

    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reconcile/{account_id}/sessions/{session_id}"
    ))
    .into_response())
}

/// POST unreconcile: reopen a closed session with a reason (min 10
/// chars, audit-logged). Lines return to uncleared.
pub async fn unreconcile(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, session_id)): Path<(Uuid, Uuid, Uuid)>,
    Form(form): Form<UnreconcileForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let session = load_session(&state, ledger_id, account_id, session_id).await?;

    let reason = form.reason.unwrap_or_default();
    domain::validate_unreconcile_reason(&reason).map_err(AppError::Validation)?;
    if session.status != "closed" {
        return Err(AppError::Validation("session is not closed".into()));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE rec_sessions SET status = 'open', closed_at = NULL, closed_by = NULL WHERE id = $1",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE rec_lines SET cleared = FALSE, cleared_at = NULL WHERE session_id = $1")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reconcile_session_unreconcile",
        "rec_session",
        Some(session_id),
        Some(serde_json::json!({ "status": "closed" })),
        Some(serde_json::json!({ "status": "open", "reason": reason })),
    )
    .await;

    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reconcile/{account_id}/sessions/{session_id}"
    ))
    .into_response())
}
