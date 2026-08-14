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
    templates::taxes::{TaxForm, TaxList, TaxRateRow, TaxReport},
    AppState,
};

#[derive(Deserialize)]
pub struct NewTaxForm {
    pub name: String,
    pub rate: String,
    pub kind: String,
    pub account_id: String,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let rates = sqlx::query_as::<_, TaxRateRow>(
        r#"SELECT id, name, rate, kind, account_id, is_active
           FROM tax_rates
           WHERE ledger_id = $1
           ORDER BY kind, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TaxList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        rates,
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
        r#"SELECT id, code, name FROM accounts
           WHERE ledger_id = $1 AND is_archived = FALSE
                 AND type IN ('LIABILITY', 'ASSET')
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TaxForm {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        accounts,
        name: String::new(),
        rate: String::new(),
        kind: "sales_tax".to_string(),
        account_id: String::new(),
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewTaxForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }

    let rate: Decimal = form
        .rate
        .parse()
        .map_err(|_| AppError::Validation("Invalid rate".into()))?;
    if rate < Decimal::ZERO || rate > Decimal::ONE {
        return Err(AppError::Validation("Rate must be between 0 and 1".into()));
    }

    let kind = form.kind.clone();
    if !["sales_tax", "purchase_tax"].contains(&kind.as_str()) {
        return Err(AppError::Validation("Invalid kind".into()));
    }

    let account_id = Uuid::parse_str(&form.account_id)
        .map_err(|_| AppError::Validation("Invalid account".into()))?;

    let rate_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO tax_rates (ledger_id, name, rate, kind, account_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(rate)
    .bind(&kind)
    .bind(account_id)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "tax_rate",
        Some(rate_id),
        None,
        Some(serde_json::json!({
            "name": name,
            "rate": rate.to_string(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/taxes", ledger_id)).into_response())
}

pub async fn toggle(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, rate_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    sqlx::query("UPDATE tax_rates SET is_active = NOT is_active WHERE id = $1")
        .bind(rate_id)
        .execute(&state.pool)
        .await?;

    Ok(Redirect::to(&format!("/ledgers/{}/taxes", ledger_id)).into_response())
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
    let from_date = NaiveDate::parse_from_str(&from_str, "%Y-%m-%d").ok();
    let to_date = NaiveDate::parse_from_str(&to_str, "%Y-%m-%d").ok();

    let rows = sqlx::query_as::<_, (Uuid, String, String, Decimal, Decimal, i64)>(
        r#"SELECT tr.id, tr.name, tr.kind, tr.rate,
                  COALESCE(SUM(pt.tax_amount), 0) AS total_tax,
                  COUNT(DISTINCT pt.posting_id) AS txn_count
           FROM tax_rates tr
           LEFT JOIN posting_taxes pt ON pt.tax_rate_id = tr.id
           LEFT JOIN postings p ON p.id = pt.posting_id
           LEFT JOIN transactions t ON t.id = p.transaction_id
           WHERE tr.ledger_id = $1
                 AND ($2::date IS NULL OR t.txn_date >= $2)
                 AND ($3::date IS NULL OR t.txn_date <= $3)
           GROUP BY tr.id
           ORDER BY tr.kind, tr.name"#,
    )
    .bind(ledger_id)
    .bind(from_date)
    .bind(to_date)
    .fetch_all(&state.pool)
    .await?;

    let summary: Vec<(Uuid, String, String, Decimal, Decimal, i64)> = rows;

    Ok(render_response(TaxReport {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        from: from_str,
        to: to_str,
        summary,
    }))
}

pub async fn export_csv(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let from_date = params
        .get("from")
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let to_date = params
        .get("to")
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let rows = sqlx::query_as::<
        _,
        (
            chrono::NaiveDate,
            Option<String>,
            Option<String>,
            Option<String>,
            Decimal,
            String,
            Decimal,
        ),
    >(
        r#"SELECT t.txn_date,
                  i.number AS invoice_number,
                  c.name AS contact_name,
                  t.payee AS payee,
                  p.amount AS taxable_amount,
                  tr.name AS tax_name,
                  pt.tax_amount
           FROM posting_taxes pt
           JOIN postings p ON p.id = pt.posting_id
           JOIN transactions t ON t.id = p.transaction_id
           JOIN tax_rates tr ON tr.id = pt.tax_rate_id
           LEFT JOIN invoices i ON i.id = t.invoice_id
           LEFT JOIN contacts c ON c.id = t.contact_id
           WHERE t.ledger_id = $1
                 AND ($2::date IS NULL OR t.txn_date >= $2)
                 AND ($3::date IS NULL OR t.txn_date <= $3)
           ORDER BY t.txn_date, t.created_at"#,
    )
    .bind(ledger_id)
    .bind(from_date)
    .bind(to_date)
    .fetch_all(&state.pool)
    .await?;

    let mut csv = String::from("date,invoice,contact,taxable_amount,tax_rate_name,tax_amount\n");
    for (date, inv, contact, _payee, taxable, name, tax_amount) in rows {
        let party = contact.unwrap_or_default();
        let invoice_num = inv.unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            date.format("%Y-%m-%d"),
            invoice_num,
            party,
            taxable,
            name,
            tax_amount
        ));
    }

    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/csv; charset=utf-8")
        .header("content-disposition", "attachment; filename=tax-export.csv")
        .body(axum::body::Body::from(csv))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}
