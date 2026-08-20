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
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::payments::{PaymentForm, PaymentList, PaymentRegister, PaymentRow},
    AppState,
};

#[derive(Deserialize)]
pub struct NewPaymentForm {
    pub contact_id: String,
    pub invoice_id: String,
    pub amount: String,
    pub payment_date: String,
    pub payment_method: String,
    pub reference: Option<String>,
    pub kind: String,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let unapplied = sqlx::query_as::<_, PaymentRow>(
        r#"SELECT p.id, p.amount, p.payment_date, p.payment_method, COALESCE(p.reference, '') AS reference,
                  p.kind, p.contact_id, p.invoice_id, p.transaction_id,
                  COALESCE(c.name, '') AS contact_name,
                  COALESCE(i.number, '') AS invoice_number,
                  (p.invoice_id IS NULL) AS unapplied
           FROM payments p
           LEFT JOIN contacts c ON c.id = p.contact_id
           LEFT JOIN invoices i ON i.id = p.invoice_id
           WHERE p.ledger_id = $1 AND p.invoice_id IS NULL
           ORDER BY p.payment_date DESC, p.created_at DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(PaymentList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        unapplied,
        error: String::new(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let contacts = sqlx::query_as::<_, (Uuid, String)>(
        r#"SELECT id, name FROM contacts WHERE ledger_id = $1 ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // `a18-invoicing-upgrade`: the column is `invoice_number`, not
    // `number` (this query previously 500'd whenever the page loaded).
    let invoices = sqlx::query_as::<_, (Uuid, String, Decimal)>(
        r#"SELECT id, invoice_number, total FROM invoices
           WHERE ledger_id = $1 AND status NOT IN ('paid', 'void')
           ORDER BY invoice_number DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // `a18-invoicing-upgrade`: allow the invoice detail page to
    // pre-open the payment form for a specific invoice.
    let invoice_id = q.get("invoice_id").cloned().unwrap_or_default();

    Ok(render_response(PaymentForm {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        contacts,
        invoices,
        amount: String::new(),
        payment_date: chrono::Utc::now().date_naive().to_string(),
        payment_method: "bank_transfer".to_string(),
        reference: String::new(),
        kind: "received".to_string(),
        contact_id: String::new(),
        invoice_id,
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewPaymentForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let amount: Decimal = form
        .amount
        .parse()
        .map_err(|_| AppError::Validation("Invalid amount".into()))?;
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("Amount must be positive".into()));
    }

    let payment_date = NaiveDate::parse_from_str(&form.payment_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;

    let contact_id = if form.contact_id.is_empty() {
        None
    } else {
        Some(
            Uuid::parse_str(&form.contact_id)
                .map_err(|_| AppError::Validation("Invalid contact".into()))?,
        )
    };

    let invoice_id = if form.invoice_id.is_empty() {
        None
    } else {
        Some(
            Uuid::parse_str(&form.invoice_id)
                .map_err(|_| AppError::Validation("Invalid invoice".into()))?,
        )
    };

    let kind = form.kind.clone();
    if !["received", "made"].contains(&kind.as_str()) {
        return Err(AppError::Validation("Invalid kind".into()));
    }

    let payment_method = form.payment_method.clone();
    if !["cash", "check", "bank_transfer", "credit_card", "other"]
        .contains(&payment_method.as_str())
    {
        return Err(AppError::Validation("Invalid payment method".into()));
    }

    let mut tx = state.pool.begin().await?;

    // Create the transaction that this payment corresponds to.
    let txn_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, kind, currency, contact_id, invoice_id, created_by)
           VALUES ($1, $2, $3, $4, $5, 'standard',
                   (SELECT base_currency FROM ledgers WHERE id = $1), $6, $7, $8)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(payment_date)
    .bind(format!("Payment {} - {}", kind, payment_method))
    .bind(form.reference.as_deref().filter(|p| !p.is_empty()))
    .bind(form.reference.as_deref().filter(|p| !p.is_empty()))
    .bind(contact_id)
    .bind(invoice_id)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    // Determine the appropriate accounts.
    let (cash_account, contra_account, contra_type): (Uuid, Uuid, String) = if let Some(inv_id) =
        invoice_id
    {
        let invoice = sqlx::query_as::<_, (String, String)>(
            r#"SELECT contact_id::text, kind FROM invoices WHERE id = $1"#,
        )
        .bind(inv_id)
        .fetch_one(&mut *tx)
        .await?;

        let contact_id_from_invoice: Uuid = invoice
            .0
            .parse()
            .map_err(|_| AppError::Internal("Invalid contact id".into()))?;
        let invoice_kind = invoice.1;

        let cash_account: Uuid = if invoice_kind == "receivable" {
            sqlx::query_scalar(
                r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' AND subtype = 'cash' LIMIT 1"#,
            )
        } else {
            sqlx::query_scalar(
                r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'LIABILITY' AND name = 'Accounts Payable' LIMIT 1"#,
            )
        }
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

        let contra_account: Uuid = if invoice_kind == "receivable" {
            sqlx::query_scalar(
                r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' AND name = 'Accounts Receivable' LIMIT 1"#,
            )
        } else {
            sqlx::query_scalar(
                r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' AND subtype = 'cash' LIMIT 1"#,
            )
        }
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

        let _ = contact_id_from_invoice;
        (cash_account, contra_account, invoice_kind)
    } else {
        // No invoice: just use cash and a placeholder.
        let cash_account: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' AND subtype = 'cash' LIMIT 1"#,
        )
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

        let contra_account: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'REVENUE' LIMIT 1"#,
        )
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

        (cash_account, contra_account, "receivable".to_string())
    };

    // For received payments: debit cash, credit AR/Revenue
    // For made payments: credit cash, debit AP/Expense
    let (debit_account, credit_account) = if kind == "received" {
        (cash_account, contra_account)
    } else {
        (contra_account, cash_account)
    };

    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency, memo)
           VALUES ($1, $2, 'DEBIT', $3,
                   (SELECT base_currency FROM ledgers WHERE id = $4), $5)"#,
    )
    .bind(txn_id)
    .bind(debit_account)
    .bind(amount)
    .bind(ledger_id)
    .bind(form.reference.as_deref().filter(|p| !p.is_empty()))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency, memo)
           VALUES ($1, $2, 'CREDIT', $3,
                   (SELECT base_currency FROM ledgers WHERE id = $4), $5)"#,
    )
    .bind(txn_id)
    .bind(credit_account)
    .bind(amount)
    .bind(ledger_id)
    .bind(form.reference.as_deref().filter(|p| !p.is_empty()))
    .execute(&mut *tx)
    .await?;

    // Create the payment record.
    let payment_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO payments (ledger_id, contact_id, invoice_id, transaction_id, amount, payment_date, payment_method, reference, kind)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .bind(invoice_id)
    .bind(txn_id)
    .bind(amount)
    .bind(payment_date)
    .bind(&payment_method)
    .bind(form.reference.as_deref().filter(|p| !p.is_empty()))
    .bind(&kind)
    .fetch_one(&mut *tx)
    .await?;

    // Apply to invoice if specified.
    if let Some(inv_id) = invoice_id {
        let invoice_total: Decimal = sqlx::query_scalar("SELECT total FROM invoices WHERE id = $1")
            .bind(inv_id)
            .fetch_one(&mut *tx)
            .await?;
        let new_amount_paid: Decimal =
            sqlx::query_scalar("SELECT COALESCE(amount_paid, 0) + $1 FROM invoices WHERE id = $2")
                .bind(amount)
                .bind(inv_id)
                .fetch_one(&mut *tx)
                .await?;

        let new_status = if new_amount_paid >= invoice_total {
            "paid"
        } else {
            "partial"
        };
        sqlx::query(
            "UPDATE invoices SET amount_paid = $1, status = $2, updated_at = now() WHERE id = $3",
        )
        .bind(new_amount_paid)
        .bind(new_status)
        .bind(inv_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "payment",
        Some(payment_id),
        None,
        Some(serde_json::json!({
            "amount": amount.to_string(),
            "kind": kind,
            "method": payment_method,
        })),
    )
    .await;

    let _ = contra_type;
    Ok(Redirect::to(&format!("/ledgers/{}/payments", ledger_id)).into_response())
}

pub async fn register(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let payments = sqlx::query_as::<_, PaymentRow>(
        r#"SELECT p.id, p.amount, p.payment_date, p.payment_method, COALESCE(p.reference, '') AS reference,
                  p.kind, p.contact_id, p.invoice_id, p.transaction_id,
                  COALESCE(c.name, '') AS contact_name,
                  COALESCE(i.number, '') AS invoice_number,
                  (p.invoice_id IS NULL) AS unapplied
           FROM payments p
           LEFT JOIN contacts c ON c.id = p.contact_id
           LEFT JOIN invoices i ON i.id = p.invoice_id
           WHERE p.ledger_id = $1
           ORDER BY p.payment_date DESC, p.created_at DESC
           LIMIT 200"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(PaymentRegister {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        payments,
    }))
}

pub async fn apply(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, payment_id)): Path<(Uuid, Uuid)>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let invoice_id = params
        .get("invoice_id")
        .ok_or(AppError::Validation("Missing invoice_id".into()))?;
    let invoice_id = Uuid::parse_str(invoice_id)
        .map_err(|_| AppError::Validation("Invalid invoice_id".into()))?;

    let mut tx = state.pool.begin().await?;

    let amount: Decimal = sqlx::query_scalar("SELECT amount FROM payments WHERE id = $1")
        .bind(payment_id)
        .fetch_one(&mut *tx)
        .await?;

    let invoice_total: Decimal = sqlx::query_scalar("SELECT total FROM invoices WHERE id = $1")
        .bind(invoice_id)
        .fetch_one(&mut *tx)
        .await?;
    let new_amount_paid: Decimal =
        sqlx::query_scalar("SELECT COALESCE(amount_paid, 0) + $1 FROM invoices WHERE id = $2")
            .bind(amount)
            .bind(invoice_id)
            .fetch_one(&mut *tx)
            .await?;

    let new_status = if new_amount_paid >= invoice_total {
        "paid"
    } else {
        "partial"
    };
    sqlx::query("UPDATE payments SET invoice_id = $1 WHERE id = $2")
        .bind(invoice_id)
        .bind(payment_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE invoices SET amount_paid = $1, status = $2, updated_at = now() WHERE id = $3",
    )
    .bind(new_amount_paid)
    .bind(new_status)
    .bind(invoice_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "apply",
        "payment",
        Some(payment_id),
        None,
        Some(serde_json::json!({
            "invoice_id": invoice_id.to_string(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/payments", ledger_id)).into_response())
}
