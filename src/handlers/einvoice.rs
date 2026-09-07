//! E-invoice export endpoints (`e-invoicing-facturx`).
//!
//! `GET /ledgers/{id}/invoices/{iid}/export.xml?format=ubl|facturx`
//! returns UBL 2.1 or CII (Factur-X payload / XRechnung) XML. Missing
//! mandatory profile fields produce a 422 listing exactly which fields
//! to fill in.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    auth::Backend,
    domain::einvoice::{
        cii_xml, ubl_xml, EinvoiceData, EinvoiceLine, EinvoiceParty, MissingFields,
    },
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

async fn load_invoice_data(
    state: &AppState,
    ledger_id: Uuid,
    invoice_id: Uuid,
) -> AppResult<EinvoiceData> {
    let inv: Option<(String, Option<String>, NaiveDate, NaiveDate, Decimal, Uuid)> =
        sqlx::query_as(
            "SELECT doc_kind, invoice_number, invoice_date, due_date, total, contact_id
         FROM invoices WHERE id = $1 AND ledger_id = $2 AND doc_kind = 'invoice'",
        )
        .bind(invoice_id)
        .bind(ledger_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some((_kind, number, date, due_date, total, contact_id)) = inv else {
        return Err(AppError::NotFound);
    };

    let lines: Vec<(String, Decimal, Decimal, Decimal)> = sqlx::query_as(
        "SELECT description, quantity, unit_price, amount FROM invoice_lines
         WHERE invoice_id = $1 ORDER BY sort_order",
    )
    .bind(invoice_id)
    .fetch_all(&state.pool)
    .await?;

    let party = |name: String,
                 vat_id: Option<String>,
                 address_line: Option<String>,
                 city: Option<String>,
                 postal_code: Option<String>,
                 country_code: Option<String>| EinvoiceParty {
        name,
        vat_id,
        address_line,
        city,
        postal_code,
        country_code,
    };

    let seller: (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT name, vat_id, address_line, city, postal_code, country_code
         FROM ledgers WHERE id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;
    // Seller identity comes from the ledger; VAT/address fields live on
    // contacts only today. The ledger owner's details are stored on a
    // self-contact when present, else blank (→ validation error lists
    // what is missing).
    let seller_party = party(seller.0, seller.1, seller.2, seller.3, seller.4, seller.5);

    let buyer: (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT name, vat_id, address_line, city, postal_code, country_code
             FROM contacts WHERE id = $1",
    )
    .bind(contact_id)
    .fetch_one(&state.pool)
    .await?;
    let buyer_party = party(buyer.0, buyer.1, buyer.2, buyer.3, buyer.4, buyer.5);

    let (base_currency,): (String,) =
        sqlx::query_as("SELECT base_currency FROM ledgers WHERE id = $1")
            .bind(ledger_id)
            .fetch_one(&state.pool)
            .await?;

    let (payment_terms, payment_means): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT payment_terms, payment_means_code FROM invoices WHERE id = $1")
            .bind(invoice_id)
            .fetch_one(&state.pool)
            .await?;

    Ok(EinvoiceData {
        number: number.unwrap_or_else(|| format!("INV-{invoice_id}")),
        date,
        due_date,
        currency: base_currency,
        seller: seller_party,
        buyer: buyer_party,
        lines: lines
            .into_iter()
            .map(|(description, quantity, unit_price, amount)| EinvoiceLine {
                description,
                quantity,
                unit_price,
                amount,
            })
            .collect(),
        total_net: total,
        payment_terms,
        payment_means_code: payment_means,
    })
}

pub async fn export_xml(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let data = load_invoice_data(&state, ledger_id, invoice_id).await?;
    let format = params.get("format").map(String::as_str).unwrap_or("ubl");
    let result = match format {
        "facturx" | "cii" => cii_xml(&data),
        "ubl" => ubl_xml(&data),
        other => {
            return Err(AppError::Validation(format!(
                "unknown format '{other}', expected 'ubl' or 'facturx'"
            )))
        }
    };
    let xml = result.map_err(|MissingFields(fields)| {
        AppError::Validation(format!(
            "missing mandatory e-invoice fields: {}",
            fields.join(", ")
        ))
    })?;

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/xml".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{invoice_id}.{format}.xml\""),
            ),
        ],
        xml,
    )
        .into_response())
}
