//! Estimates (quotes) with convert-to-invoice (`invoicing-completeness`).

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
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
    AppState,
};

#[derive(Deserialize)]
pub struct NewEstimateForm {
    pub contact_id: String,
    pub invoice_date: String,
    pub due_date: String,
    pub description: Vec<String>,
    pub quantity: Vec<String>,
    pub unit_price: Vec<String>,
}

/// Parsed line-oriented estimate body. Line fields repeat per row,
/// which `serde_urlencoded` cannot express for `Vec<String>` — parse
/// the urlencoded body directly (same approach as webhooks_out).
struct EstimateBody {
    contact_id: String,
    invoice_date: String,
    due_date: String,
    description: Vec<String>,
    quantity: Vec<String>,
    unit_price: Vec<String>,
}

impl EstimateBody {
    fn parse(body: &str) -> Self {
        let mut out = EstimateBody {
            contact_id: String::new(),
            invoice_date: String::new(),
            due_date: String::new(),
            description: Vec::new(),
            quantity: Vec::new(),
            unit_price: Vec::new(),
        };
        for (k, v) in form_urlencoded::parse(body.as_bytes()) {
            match k.as_ref() {
                "contact_id" => out.contact_id = v.into_owned(),
                "invoice_date" => out.invoice_date = v.into_owned(),
                "due_date" => out.due_date = v.into_owned(),
                "description" => out.description.push(v.into_owned()),
                "quantity" => out.quantity.push(v.into_owned()),
                "unit_price" => out.unit_price.push(v.into_owned()),
                _ => {}
            }
        }
        out
    }
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    body: String,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let form = EstimateBody::parse(&body);

    let contact_id = Uuid::parse_str(&form.contact_id)
        .map_err(|_| AppError::Validation("Invalid contact".into()))?;
    let invoice_date = NaiveDate::parse_from_str(&form.invoice_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;
    let due_date = NaiveDate::parse_from_str(&form.due_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid due date".into()))?;

    let mut lines: Vec<(String, Decimal, Decimal)> = Vec::new();
    for ((d, q), up) in form
        .description
        .iter()
        .zip(&form.quantity)
        .zip(&form.unit_price)
    {
        let desc = d.trim();
        if desc.is_empty() {
            continue;
        }
        let qty: Decimal = q
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("Invalid quantity".into()))?;
        let price: Decimal = up
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("Invalid unit price".into()))?;
        lines.push((desc.to_string(), qty, price));
    }
    if lines.is_empty() {
        return Err(AppError::Validation(
            "An estimate needs at least one line".into(),
        ));
    }
    let total: Decimal = lines.iter().map(|(_, q, p)| (*q * *p).round_dp(2)).sum();

    let mut tx = state.pool.begin().await?;
    let (estimate_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, doc_kind, invoice_date, due_date,
                                 total, status)
           VALUES ($1, $2, 'receivable', 'estimate', $3, $4, $5, 'draft')
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .bind(invoice_date)
    .bind(due_date)
    .bind(total)
    .fetch_one(&mut *tx)
    .await?;
    for (i, (description, quantity, unit_price)) in lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(estimate_id)
        .bind(description)
        .bind(quantity)
        .bind(unit_price)
        .bind((*quantity * *unit_price).round_dp(2))
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
        "estimate",
        Some(estimate_id),
        None,
        Some(serde_json::json!({ "total": total.to_string(), "lines": lines.len() })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/estimates")).into_response())
}

/// Convert an accepted estimate into an invoice. Idempotent by the
/// `converted` status: a second call is a 409.
pub async fn convert(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, estimate_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    // Claim the estimate atomically: only one caller flips it to converted.
    let claimed: Option<(
        Uuid,
        Uuid,
        String,
        Option<String>,
        NaiveDate,
        NaiveDate,
        Decimal,
    )> = sqlx::query_as(
        r#"UPDATE invoices SET status = 'converted', updated_at = now()
               WHERE id = $1 AND ledger_id = $2 AND doc_kind = 'estimate'
                     AND status NOT IN ('converted', 'declined')
               RETURNING id, contact_id, kind, invoice_number, invoice_date, due_date, total"#,
    )
    .bind(estimate_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((_id, contact_id, _kind, number, invoice_date, due_date, total)) = claimed else {
        return Err(AppError::Conflict(
            "estimate already converted or declined".into(),
        ));
    };

    let mut tx = state.pool.begin().await?;
    let (invoice_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number, invoice_date,
                                 due_date, total, status, doc_kind)
           VALUES ($1, $2, 'receivable', $3, $4, $5, $6, 'open', 'invoice')
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .bind(number)
    .bind(invoice_date)
    .bind(due_date)
    .bind(total)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
         SELECT $1, description, quantity, unit_price, amount, sort_order
         FROM invoice_lines WHERE invoice_id = $2 ORDER BY sort_order",
    )
    .bind(invoice_id)
    .bind(estimate_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "convert",
        "estimate",
        Some(estimate_id),
        None,
        Some(serde_json::json!({ "invoice_id": invoice_id })),
    )
    .await;

    crate::jobs::events::emit(
        &state.pool,
        ledger_id,
        "invoice.created",
        serde_json::json!({ "invoice_id": invoice_id, "from_estimate": estimate_id }),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/invoices/{invoice_id}")).into_response())
}
