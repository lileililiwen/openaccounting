use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::{Contact, Invoice},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::invoices::{InvoiceList, InvoiceNew},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let kind_filter = q.get("kind").map(|s| s.as_str()).unwrap_or("all");

    let invoices = sqlx::query_as::<_, Invoice>(
        r#"SELECT id, ledger_id, contact_id, kind, invoice_number, invoice_date, due_date, total, amount_paid, status, created_at, updated_at
           FROM invoices WHERE ledger_id = $1 AND ($2 = 'all' OR kind = $2)
           ORDER BY due_date ASC"#,
    )
    .bind(ledger_id)
    .bind(kind_filter)
    .fetch_all(&state.pool)
    .await?;

    // Get contact names
    let mut invoices_with_contacts = Vec::new();
    for inv in invoices {
        let contact_name: Option<String> =
            sqlx::query_scalar("SELECT name FROM contacts WHERE id = $1")
                .bind(inv.contact_id)
                .fetch_optional(&state.pool)
                .await
                .unwrap_or(None);
        invoices_with_contacts.push((inv, contact_name.unwrap_or_default()));
    }

    Ok(render_response(InvoiceList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        invoices: invoices_with_contacts,
        kind_filter: kind_filter.to_string(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let contacts = sqlx::query_as::<_, Contact>(
        r#"SELECT id, ledger_id, name, email, phone, kind, created_at, updated_at
           FROM contacts WHERE ledger_id = $1 ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(InvoiceNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        contacts,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewInvoiceForm {
    pub contact_id: Uuid,
    pub kind: String,
    pub invoice_number: Option<String>,
    pub invoice_date: String,
    pub due_date: String,
    pub total: String,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewInvoiceForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let contacts = sqlx::query_as::<_, Contact>(
        r#"SELECT id, ledger_id, name, email, phone, kind, created_at, updated_at
           FROM contacts WHERE ledger_id = $1 ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let make_error = |msg: String| InvoiceNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        contacts: contacts.clone(),
        error: msg,
    };

    let kind = match form.kind.as_str() {
        "receivable" | "payable" => form.kind.clone(),
        _ => return Err(AppError::Validation("Invalid invoice kind".into())),
    };

    let invoice_date = NaiveDate::parse_from_str(&form.invoice_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid invoice date".into()))?;
    let due_date = NaiveDate::parse_from_str(&form.due_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid due date".into()))?;

    let total: Decimal = form
        .total
        .parse()
        .map_err(|_| AppError::Validation("Invalid total amount".into()))?;

    if total <= Decimal::ZERO {
        return Err(AppError::Validation(
            "Total must be greater than zero".into(),
        ));
    }

    let result = sqlx::query(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number, invoice_date, due_date, total)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(form.contact_id)
    .bind(&kind)
    .bind(form.invoice_number.as_deref().filter(|s| !s.is_empty()))
    .bind(invoice_date)
    .bind(due_date)
    .bind(total)
    .fetch_one(&state.pool)
    .await?;

    let invoice_id: Uuid = result.get(0);
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "invoice",
        Some(invoice_id),
        None,
        Some(serde_json::json!({
            "kind": kind,
            "total": total,
            "due_date": due_date
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/invoices", ledger_id)).into_response())
}
