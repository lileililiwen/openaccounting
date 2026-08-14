//! HTTP handlers for the expense reimbursement module.

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    domain::reimbursement::{format_short_id, ClaimStatus},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::reimbursement::{ClaimList, ClaimListRow, ClaimNew, ClaimShow, ClaimShowLine},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let rows: Vec<ClaimListRow> = sqlx::query_as(
        r#"SELECT id, short_id, title, employee_name, status, currency,
                  COALESCE((SELECT SUM(amount) FROM reimbursement_lines
                            WHERE claim_id = reimbursement_claims.id), 0) AS total
           FROM reimbursement_claims
           WHERE ledger_id = $1
           ORDER BY created_at DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(ClaimList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        claims: rows,
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
    Ok(render_response(ClaimNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewClaimForm {
    pub title: String,
    pub description: Option<String>,
    pub currency: String,
    pub employee_name: String,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewClaimForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    if form.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if form.currency.len() != 3 {
        return Err(AppError::Validation(
            "currency must be a 3-letter code".into(),
        ));
    }
    if form.employee_name.trim().is_empty() {
        return Err(AppError::Validation("employee_name is required".into()));
    }
    if form.currency.to_uppercase() != ledger.base_currency.to_uppercase() {
        return Err(AppError::Validation(format!(
            "currency mismatch: claim '{}' vs ledger base '{}'",
            form.currency.to_uppercase(),
            ledger.base_currency
        )));
    }
    let new_id = Uuid::new_v4();
    let short = format_short_id(&new_id);
    sqlx::query(
        r#"INSERT INTO reimbursement_claims
              (id, ledger_id, short_id, employee_id, employee_name,
               title, description, currency, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'draft')"#,
    )
    .bind(new_id)
    .bind(ledger_id)
    .bind(&short)
    .bind(user.id)
    .bind(form.employee_name.trim())
    .bind(form.title.trim())
    .bind(form.description.as_deref().unwrap_or(""))
    .bind(form.currency.to_uppercase())
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO reimbursement_events (claim_id, actor_id, event_type, payload)
           VALUES ($1, $2, 'create', '{}'::jsonb)"#,
    )
    .bind(new_id)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reimbursement.create",
        "reimbursement_claim",
        Some(new_id),
        None,
        Some(serde_json::json!({"title": form.title})),
    )
    .await;
    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reimbursements/{new_id}"
    ))
    .into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let claim = load_claim(&state, claim_id, ledger_id).await?;
    let lines: Vec<ClaimShowLine> = sqlx::query_as(
        r#"SELECT id, txn_date, description, amount, gl_account_id, tax_amount, advance_amount
           FROM reimbursement_lines WHERE claim_id = $1 ORDER BY id"#,
    )
    .bind(claim_id)
    .fetch_all(&state.pool)
    .await?;
    let total: Decimal = lines.iter().map(|l| l.amount).sum();
    Ok(render_response(ClaimShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        claim,
        lines,
        total,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewLineForm {
    pub txn_date: String,
    pub description: String,
    pub amount: String,
    pub gl_account_id: Uuid,
    pub tax_amount: Option<String>,
    pub advance_amount: Option<String>,
}

pub async fn add_line(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<NewLineForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let claim = load_claim(&state, claim_id, ledger_id).await?;
    if claim.status != ClaimStatus::Draft {
        return Err(AppError::Validation(format!(
            "claim is in status '{}', only drafts accept new lines",
            claim.status.as_str()
        )));
    }
    let amount: Decimal = form
        .amount
        .parse()
        .map_err(|e| AppError::Validation(format!("bad amount: {e}")))?;
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("amount must be positive".into()));
    }
    let tax: Decimal = form
        .tax_amount
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(|e| AppError::Validation(format!("bad tax_amount: {e}")))?;
    let advance: Decimal = form
        .advance_amount
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(|e| AppError::Validation(format!("bad advance_amount: {e}")))?;
    let txn_date = NaiveDate::parse_from_str(&form.txn_date, "%Y-%m-%d")
        .map_err(|e| AppError::Validation(format!("bad date: {e}")))?;
    // Validate that the GL account is an EXPENSE subtype.
    let (gl_subtype,): (String,) = sqlx::query_as(
        "SELECT subtype FROM accounts WHERE id = $1 AND ledger_id = $2",
    )
    .bind(form.gl_account_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Validation("gl_account not in ledger".into()))?;
    if !matches!(
        gl_subtype.as_str(),
        "OPERATING_EXPENSE" | "COST_OF_GOODS_SOLD" | "NON_OPERATING_EXPENSE" | "TAX_EXPENSE"
    ) {
        return Err(AppError::Validation(format!(
            "gl account subtype '{gl_subtype}' is not an EXPENSE"
        )));
    }
    sqlx::query(
        r#"INSERT INTO reimbursement_lines
              (claim_id, txn_date, description, amount,
               gl_account_id, tax_amount, advance_amount)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(claim_id)
    .bind(txn_date)
    .bind(&form.description)
    .bind(amount)
    .bind(form.gl_account_id)
    .bind(tax)
    .bind(advance)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reimbursements/{claim_id}"
    ))
    .into_response())
}

pub async fn submit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let claim = load_claim(&state, claim_id, ledger_id).await?;
    if !claim.status.can_transition_to(ClaimStatus::Submitted) {
        return Err(AppError::Validation(format!(
            "claim cannot be submitted from status '{}'",
            claim.status.as_str()
        )));
    }
    let (line_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM reimbursement_lines WHERE claim_id = $1",
    )
    .bind(claim_id)
    .fetch_one(&state.pool)
    .await?;
    if line_count == 0 {
        return Err(AppError::Validation(
            "cannot submit an empty claim".into(),
        ));
    }
    update_status(&state, ledger_id, claim_id, user.id, ClaimStatus::Submitted, None).await?;
    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reimbursements/{claim_id}"
    ))
    .into_response())
}

#[derive(Deserialize)]
pub struct RejectForm {
    pub reason: String,
}

pub async fn reject(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<RejectForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let claim = load_claim(&state, claim_id, ledger_id).await?;
    if !claim.status.can_transition_to(ClaimStatus::Rejected) {
        return Err(AppError::Validation(format!(
            "claim cannot be rejected from status '{}'",
            claim.status.as_str()
        )));
    }
    if form.reason.trim().is_empty() {
        return Err(AppError::Validation("rejection reason is required".into()));
    }
    sqlx::query(
        r#"UPDATE reimbursement_claims
           SET rejected_reason = $1, updated_at = now()
           WHERE id = $2 AND ledger_id = $3"#,
    )
    .bind(form.reason.trim())
    .bind(claim_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    update_status(
        &state,
        ledger_id,
        claim_id,
        user.id,
        ClaimStatus::Rejected,
        Some(serde_json::json!({"reason": form.reason})),
    )
    .await?;
    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reimbursements/{claim_id}"
    ))
    .into_response())
}

pub async fn approve(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let claim = load_claim(&state, claim_id, ledger_id).await?;
    if !claim.status.can_transition_to(ClaimStatus::Approved) {
        // Spec: a second approve returns 409.
        if claim.status == ClaimStatus::Approved {
            return Err(AppError::Conflict(
                "claim is already approved".into(),
            ));
        }
        return Err(AppError::Validation(format!(
            "claim cannot be approved from status '{}'",
            claim.status.as_str()
        )));
    }
    let mut tx = state.pool.begin().await?;
    let total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM reimbursement_lines WHERE claim_id = $1",
    )
    .bind(claim_id)
    .fetch_one(&mut *tx)
    .await?;
    let advance_total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(advance_amount), 0) FROM reimbursement_lines WHERE claim_id = $1",
    )
    .bind(claim_id)
    .fetch_one(&mut *tx)
    .await?;
    // Sum taxable amount for the third leg (tax).
    let tax_total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(tax_amount), 0) FROM reimbursement_lines WHERE claim_id = $1",
    )
    .bind(claim_id)
    .fetch_one(&mut *tx)
    .await?;
    // Resolve the Employee Payable account.
    let (payable,): (Uuid,) = sqlx::query_as(
        r#"SELECT id FROM accounts
           WHERE ledger_id = $1 AND type = 'LIABILITY' AND subtype = 'EMPLOYEE_PAYABLE'
           LIMIT 1"#,
    )
    .bind(ledger_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::Internal(
        "no EMPLOYEE_PAYABLE account in ledger; create one before approving".into()
    ))?;
    // Group expense by GL account.
    let grouped: Vec<(Uuid, Decimal, Decimal)> = sqlx::query_as(
        r#"SELECT gl_account_id,
                  COALESCE(SUM(amount - advance_amount), 0),
                  COALESCE(SUM(tax_amount), 0)
           FROM reimbursement_lines WHERE claim_id = $1
           GROUP BY gl_account_id"#,
    )
    .bind(claim_id)
    .fetch_all(&mut *tx)
    .await?;
    if grouped.is_empty() {
        let _ = tx.rollback().await;
        return Err(AppError::Validation(
            "no lines on claim to approve".into(),
        ));
    }
    // Pick the earliest txn_date for the transaction.
    let (txn_date,): (NaiveDate,) = sqlx::query_as(
        "SELECT MIN(txn_date) FROM reimbursement_lines WHERE claim_id = $1",
    )
    .bind(claim_id)
    .fetch_one(&mut *tx)
    .await?;
    // Sum of (amount - advance) across all lines is the
    // net expense the company books.
    let net_expense: Decimal = grouped.iter().map(|(_, net, _)| *net).sum();
    // Sum of tax is the recoverable tax (if any).
    let tax_sum: Decimal = grouped.iter().map(|(_, _, t)| *t).sum();
    // Net payable = net_expense - advance_total (already
    // paid in advance by the company).
    let net_payable = net_expense - advance_total;
    // Insert one transaction per expense group + one
    // EMPLOYEE_PAYABLE credit, and a tax leg if applicable.
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions
              (ledger_id, txn_date, description, payee, reference,
               currency, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(txn_date)
    .bind(format!("Reimbursement {}", claim.short_id))
    .bind(&claim.employee_name)
    .bind(&claim.short_id)
    .bind(&claim.currency)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    for (gl_id, net, tax) in &grouped {
        // DR expense (net of advance)
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
               VALUES ($1, $2, $3, 'DEBIT')"#,
        )
        .bind(txn_id)
        .bind(gl_id)
        .bind(net)
        .execute(&mut *tx)
        .await?;
        if *tax > Decimal::ZERO {
            // DR recoverable tax into the same expense (or a
            // separate Tax Recoverable account if one exists;
            // for the v1 implementation we book it as an
            // additional tax expense).
            sqlx::query(
                r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
                   VALUES ($1, $2, $3, 'DEBIT')"#,
            )
            .bind(txn_id)
            .bind(gl_id)
            .bind(tax)
            .execute(&mut *tx)
            .await?;
        }
    }
    if advance_total > Decimal::ZERO {
        // CR Employee Advance (the pre-payment already on
        // the books; this clears it). The advance account
        // is an ASSET, so crediting it reduces the asset.
        let advance_acct: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM accounts
               WHERE ledger_id = $1 AND type = 'ASSET' AND subtype = 'EMPLOYEE_ADVANCE'
               LIMIT 1"#,
        )
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(advance_acct) = advance_acct {
            sqlx::query(
                r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
                   VALUES ($1, $2, $3, 'CREDIT')"#,
            )
            .bind(txn_id)
            .bind(advance_acct)
            .bind(advance_total)
            .execute(&mut *tx)
            .await?;
        }
    }
    // CR Employee Payable (the net amount owed).
    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
           VALUES ($1, $2, $3, 'CREDIT')"#,
    )
    .bind(txn_id)
    .bind(payable)
    .bind(net_payable)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"UPDATE reimbursement_claims
           SET status = 'approved', approved_by = $1, approved_at = now(),
               updated_at = now()
           WHERE id = $2 AND ledger_id = $3"#,
    )
    .bind(user.id)
    .bind(claim_id)
    .bind(ledger_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO reimbursement_events (claim_id, actor_id, event_type, payload)
           VALUES ($1, $2, 'approve', $3::jsonb)"#,
    )
    .bind(claim_id)
    .bind(user.id)
    .bind(serde_json::json!({"txn_id": txn_id, "total": total, "tax": tax_sum}))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reimbursement.approve",
        "reimbursement_claim",
        Some(claim_id),
        None,
        Some(serde_json::json!({"txn_id": txn_id})),
    )
    .await;
    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reimbursements/{claim_id}"
    ))
    .into_response())
}

#[derive(Deserialize)]
pub struct PayForm {
    pub payout_account_id: Uuid,
}

pub async fn pay(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<PayForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let claim = load_claim(&state, claim_id, ledger_id).await?;
    if !claim.status.can_transition_to(ClaimStatus::Paid) {
        return Err(AppError::Validation(format!(
            "claim cannot be paid from status '{}'",
            claim.status.as_str()
        )));
    }
    // Validate the payout account is a cash / bank account.
    let (payout_subtype, payout_type): (String, String) = sqlx::query_as(
        "SELECT subtype, type FROM accounts WHERE id = $1 AND ledger_id = $2",
    )
    .bind(form.payout_account_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Validation("payout account not in ledger".into()))?;
    if payout_type != "ASSET"
        || !matches!(
            payout_subtype.as_str(),
            "CURRENT_ASSET" | "OTHER_ASSET" | "EMPLOYEE_ADVANCE"
        ) && !(payout_subtype.to_lowercase().contains("cash")
            || payout_subtype.to_lowercase().contains("bank"))
    {
        return Err(AppError::Validation(format!(
            "payout account must be cash or bank (subtype={payout_subtype}, type={payout_type})"
        )));
    }
    let mut tx = state.pool.begin().await?;
    let (payable,): (Uuid,) = sqlx::query_as(
        r#"SELECT id FROM accounts
           WHERE ledger_id = $1 AND type = 'LIABILITY' AND subtype = 'EMPLOYEE_PAYABLE'
           LIMIT 1"#,
    )
    .bind(ledger_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::Internal(
        "no EMPLOYEE_PAYABLE account in ledger".into()
    ))?;
    // Find the existing approved transaction for this claim
    // so the payout references the same bookkeeping.
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"SELECT id FROM transactions
           WHERE ledger_id = $1 AND reference = $2
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(ledger_id)
    .bind(&claim.short_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::Internal(
        "no approved transaction found for claim".into()
    ))?;
    let net_payable: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(p.amount), 0)
           FROM postings p
           JOIN accounts a ON a.id = p.account_id
           WHERE p.transaction_id = $1 AND a.subtype = 'EMPLOYEE_PAYABLE'
             AND p.direction = 'CREDIT'"#,
    )
    .bind(txn_id)
    .fetch_one(&mut *tx)
    .await?;
    // Post the payout: DR Payable / CR Bank.
    sqlx::query(
        r#"INSERT INTO transactions
              (ledger_id, txn_date, description, payee, reference,
               currency, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(chrono::Utc::now().date_naive())
    .bind(format!("Reimbursement payout {}", claim.short_id))
    .bind(&claim.employee_name)
    .bind(&claim.short_id)
    .bind(&claim.currency)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let (payout_txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions
              (ledger_id, txn_date, description, payee, reference,
               currency, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(chrono::Utc::now().date_naive())
    .bind(format!("Reimbursement payout {}", claim.short_id))
    .bind(&claim.employee_name)
    .bind(&claim.short_id)
    .bind(&claim.currency)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
           VALUES ($1, $2, $3, 'DEBIT')"#,
    )
    .bind(payout_txn_id)
    .bind(payable)
    .bind(net_payable)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
           VALUES ($1, $2, $3, 'CREDIT')"#,
    )
    .bind(payout_txn_id)
    .bind(form.payout_account_id)
    .bind(net_payable)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"UPDATE reimbursement_claims
           SET status = 'paid', paid_at = now(), payout_account_id = $1,
               updated_at = now()
           WHERE id = $2 AND ledger_id = $3"#,
    )
    .bind(form.payout_account_id)
    .bind(claim_id)
    .bind(ledger_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO reimbursement_events (claim_id, actor_id, event_type, payload)
           VALUES ($1, $2, 'pay', $3::jsonb)"#,
    )
    .bind(claim_id)
    .bind(user.id)
    .bind(serde_json::json!({"payout_txn_id": payout_txn_id}))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reimbursement.pay",
        "reimbursement_claim",
        Some(claim_id),
        None,
        None,
    )
    .await;
    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/reimbursements/{claim_id}"
    ))
    .into_response())
}

async fn load_claim(
    state: &AppState,
    claim_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<crate::domain::reimbursement::Claim> {
    let row: Option<(Uuid, Uuid, String, Uuid, String, String, Option<String>, String, String, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<String>, Option<chrono::DateTime<chrono::Utc>>, Option<Uuid>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"SELECT id, ledger_id, short_id, employee_id, employee_name,
                  title, description, currency, status, approved_by,
                  approved_at, rejected_reason, paid_at, payout_account_id,
                  created_at, updated_at
           FROM reimbursement_claims
           WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(claim_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((id, ledger_id, short_id, employee_id, employee_name, title, description, currency, status, approved_by, approved_at, rejected_reason, paid_at, payout_account_id, created_at, updated_at)) = row else {
        return Err(AppError::NotFound);
    };
    Ok(crate::domain::reimbursement::Claim {
        id,
        ledger_id,
        short_id,
        employee_id,
        employee_name,
        title,
        description,
        currency,
        status: ClaimStatus::parse(&status).ok_or_else(|| AppError::Internal("bad status".into()))?,
        approved_by,
        approved_at,
        rejected_reason,
        paid_at,
        payout_account_id,
        created_at,
        updated_at,
    })
}

async fn update_status(
    state: &AppState,
    _ledger_id: Uuid,
    claim_id: Uuid,
    actor_id: Uuid,
    new_status: ClaimStatus,
    payload: Option<serde_json::Value>,
) -> AppResult<()> {
    let payload = payload.unwrap_or_else(|| serde_json::json!({}));
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        r#"UPDATE reimbursement_claims
           SET status = $1, updated_at = now()
           WHERE id = $2"#,
    )
    .bind(new_status.as_str())
    .bind(claim_id)
    .execute(&mut *tx)
    .await?;
    let event = match new_status {
        ClaimStatus::Submitted => "submit",
        ClaimStatus::Approved => "approve",
        ClaimStatus::Rejected => "reject",
        ClaimStatus::Paid => "pay",
        ClaimStatus::Draft => "create",
    };
    sqlx::query(
        r#"INSERT INTO reimbursement_events (claim_id, actor_id, event_type, payload)
           VALUES ($1, $2, $3, $4::jsonb)"#,
    )
    .bind(claim_id)
    .bind(actor_id)
    .bind(event)
    .bind(&payload)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
