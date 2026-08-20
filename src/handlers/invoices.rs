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
    templates::invoices::{InvoiceList, InvoiceNew, InvoiceShow},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

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
        current_section: "transactions".to_string(),
        invoices: invoices_with_contacts,
        kind_filter: kind_filter.to_string(),
        today: chrono::Utc::now().date_naive(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

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
        current_section: "transactions".to_string(),
        contacts,
        error: String::new(),
    }))
}

#[derive(Clone, Debug, Default)]
struct InvoiceLineInput {
    description: String,
    quantity: String,
    unit_price: String,
}

/// Parse `lines[N][description|quantity|unit_price]` from the raw form.
fn parse_lines(raw: &std::collections::HashMap<String, String>) -> Vec<InvoiceLineInput> {
    let mut by_index: std::collections::BTreeMap<usize, InvoiceLineInput> =
        std::collections::BTreeMap::new();
    for (key, value) in raw {
        if !key.starts_with("lines[") {
            continue;
        }
        let after = &key[6..];
        let Some(close) = after.find("][") else {
            continue;
        };
        let Ok(idx) = after[..close].parse::<usize>() else {
            continue;
        };
        let field = after[close + 2..].trim_end_matches(']');
        let entry = by_index.entry(idx).or_default();
        match field {
            "description" => entry.description = value.clone(),
            "quantity" => entry.quantity = value.clone(),
            "unit_price" => entry.unit_price = value.clone(),
            _ => {}
        }
    }
    by_index.into_values().collect()
}

#[derive(Deserialize)]
pub struct NewInvoiceForm {
    pub contact_id: String,
    pub kind: String,
    pub invoice_number: Option<String>,
    pub invoice_date: String,
    pub due_date: String,
    /// `lines[N][description|quantity|unit_price]` fields.
    #[serde(flatten, default)]
    pub extra: std::collections::HashMap<String, String>,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewInvoiceForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

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
        current_section: "transactions".to_string(),
        contacts: contacts.clone(),
        error: msg,
    };

    let kind = match form.kind.as_str() {
        "receivable" | "payable" => form.kind.clone(),
        _ => return Err(AppError::Validation("Invalid invoice kind".into())),
    };

    let contact_id = Uuid::parse_str(&form.contact_id)
        .map_err(|_| AppError::Validation("Invalid contact".into()))?;
    let invoice_date = NaiveDate::parse_from_str(&form.invoice_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid invoice date".into()))?;
    let due_date = NaiveDate::parse_from_str(&form.due_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid due date".into()))?;

    // Build line items; total = Σ quantity × unit_price.
    let mut lines: Vec<(String, Decimal, Decimal)> = Vec::new();
    let mut total = Decimal::ZERO;
    for line in parse_lines(&form.extra) {
        let description = line.description.trim();
        if description.is_empty() {
            continue;
        }
        let quantity: Decimal = line
            .quantity
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("Invalid line quantity".into()))?;
        let unit_price: Decimal = line
            .unit_price
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("Invalid line unit price".into()))?;
        let amount = (quantity * unit_price).round_dp(2);
        if amount <= Decimal::ZERO {
            return Err(AppError::Validation(format!(
                "Line \"{description}\" needs a positive quantity and unit price"
            )));
        }
        total += amount;
        lines.push((description.to_string(), quantity, unit_price));
    }
    if lines.is_empty() {
        return Ok(render_response(make_error(
            "Add at least one line item with a description, quantity, and unit price.".into(),
        )));
    }

    let mut tx = state.pool.begin().await?;
    let invoice_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number, invoice_date, due_date, total)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .bind(&kind)
    .bind(form.invoice_number.as_deref().filter(|s| !s.is_empty()))
    .bind(invoice_date)
    .bind(due_date)
    .bind(total)
    .fetch_one(&mut *tx)
    .await?;

    for (i, (description, quantity, unit_price)) in lines.iter().enumerate() {
        let amount = (quantity * unit_price).round_dp(2);
        sqlx::query(
            r#"INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(invoice_id)
        .bind(description)
        .bind(quantity)
        .bind(unit_price)
        .bind(amount)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

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
            "total": total.to_string(),
            "lines": lines.len(),
            "due_date": due_date,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/invoices/{invoice_id}", ledger_id)).into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let invoice = sqlx::query_as::<_, Invoice>(
        r#"SELECT id, ledger_id, contact_id, kind, invoice_number, invoice_date, due_date, total, amount_paid, status, created_at, updated_at
           FROM invoices WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let contact_name: String = sqlx::query_scalar("SELECT name FROM contacts WHERE id = $1")
        .bind(invoice.contact_id)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or_default();

    let lines = sqlx::query_as::<_, (String, Decimal, Decimal, Decimal)>(
        r#"SELECT description, quantity, unit_price, amount
           FROM invoice_lines WHERE invoice_id = $1
           ORDER BY sort_order, created_at"#,
    )
    .bind(invoice_id)
    .fetch_all(&state.pool)
    .await?;

    let outstanding = invoice.total - invoice.amount_paid;
    let overdue = invoice.status == "open"
        && invoice.due_date < chrono::Utc::now().date_naive()
        && outstanding > Decimal::ZERO;

    Ok(render_response(InvoiceShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        invoice,
        contact_name,
        lines,
        outstanding,
        overdue,
    }))
}

/// Mark an invoice fully paid (`a18-invoicing-upgrade`).
pub async fn mark_paid(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let name: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(invoice_number, '') FROM invoices WHERE id = $1 AND ledger_id = $2",
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    if name.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        r#"UPDATE invoices SET status = 'paid', amount_paid = total, updated_at = now()
           WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "mark_paid",
        "invoice",
        Some(invoice_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/invoices/{invoice_id}")).into_response())
}

/// Void an invoice (`a18-invoicing-upgrade`).
pub async fn void(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let name: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(invoice_number, '') FROM invoices WHERE id = $1 AND ledger_id = $2",
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    if name.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        r#"UPDATE invoices SET status = 'void', updated_at = now()
           WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "void",
        "invoice",
        Some(invoice_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/invoices/{invoice_id}")).into_response())
}
