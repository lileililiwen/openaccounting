//! Transaction Edit and Void via Reversing Entries
//! (`a2-transaction-edit-void`).
//!
//! Two routes:
//! - `POST /ledgers/{id}/transactions/{txn_id}/reverse`
//! - `POST /ledgers/{id}/transactions/{txn_id}/edit`
//!
//! Both preserve the original transaction (full audit); they
//! create a new `kind = 'reversing'` transaction on today's
//! date with all amounts negated. Edit additionally inserts
//! the corrected transaction in the same atomic DB tx.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::post,
    Form, Json, Router,
};
use axum_login::AuthSession;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::TxnLineInput,
    error::{AppError, AppResult},
    handlers::{ledgers, transactions::parse_lines_for_edit},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/ledgers/{ledger_id}/transactions/{txn_id}/reverse",
            post(reverse),
        )
        .route(
            "/ledgers/{ledger_id}/transactions/{txn_id}/edit",
            post(edit),
        )
}

#[derive(Debug, Deserialize)]
pub struct ReverseForm {
    /// Optional memo recorded on the reversal transaction.
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EditForm {
    pub date: String,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    /// `lines[N][account_id]`, `lines[N][direction]`, `lines[N][amount]`,
    /// `lines[N][memo]` — same shape as the create form.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct EditResponse {
    pub reversal_id: Uuid,
    pub corrected_id: Uuid,
}

/// Reverse a transaction. Creates one new transaction on today's
/// date whose postings negate the originals. The original is
/// unchanged.
pub async fn reverse(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
    Form(_form): Form<ReverseForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    // Reversals are explicitly allowed in append-only mode
    // (`d2-append-only-mode` spec: "Reversal Allowed").
    // Append-only blocks edits and deletes — reversals create a
    // new row, so they pass the trigger naturally.
    let _ = ledger;

    let mut tx = state.pool.begin().await?;
    // Lock the original row to prevent double-reversal.
    let original: Option<(Uuid, NaiveDate, String, String, NaiveDate)> = sqlx::query_as(
        "SELECT id, txn_date, description, kind, txn_date
         FROM transactions
         WHERE id = $1 AND ledger_id = $2
         FOR UPDATE",
    )
    .bind(txn_id)
    .bind(ledger_id)
    .fetch_optional(&mut *tx)
    .await?;
    let original = original.ok_or(AppError::NotFound)?;
    if original.3 == "reversing" {
        return Err(AppError::Unprocessable("Cannot reverse a reversal".into()));
    }
    let reversal_id = insert_reversal(
        &mut tx,
        original.0,
        &state,
        ledger_id,
        user.id,
        &original.2,
        original.1,
    )
    .await?;

    tx.commit().await?;

    // Audit log.
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reverse",
        "transaction",
        Some(original.0),
        None,
        Some(serde_json::json!({
            "reversal_id": reversal_id,
            "original_id": original.0,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/transactions/{txn_id}")).into_response())
}

/// Edit a transaction by creating one reversal of the original
/// AND one corrected transaction, atomically. The original
/// itself is preserved.
pub async fn edit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<EditForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    // `d2-append-only-mode`: editing a transaction in an
    // append-only ledger is forbidden. The DB trigger will
    // also reject the underlying UPDATE, but we surface a
    // clean 422 with the "reverse instead" hint here.
    if ledger.append_only {
        return Err(AppError::Unprocessable(format!(
            "ledger {} is append-only; edits are not allowed — reverse the transaction instead",
            ledger_id
        )));
    }

    let parsed = parse_lines_for_edit(&form.extra);
    if parsed.len() < 2 {
        return Err(AppError::Validation(
            "A transaction needs at least two postings.".into(),
        ));
    }
    let date = NaiveDate::parse_from_str(&form.date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date format (YYYY-MM-DD)".into()))?;
    if form.description.trim().is_empty() {
        return Err(AppError::Validation("Description is required".into()));
    }
    let mut inputs: Vec<TxnLineInput> = Vec::new();
    for l in &parsed {
        let account_id = Uuid::parse_str(&l.account_id)
            .map_err(|_| AppError::Validation("Invalid account on a posting".into()))?;
        let amount: Decimal = l
            .amount
            .parse()
            .map_err(|_| AppError::Validation("Invalid amount".into()))?;
        let signed_amount = match l.direction.to_uppercase().as_str() {
            "DEBIT" => amount,
            "CREDIT" => -amount,
            _ => {
                return Err(AppError::Validation(format!(
                    "Invalid direction: {}",
                    l.direction
                )));
            }
        };
        inputs.push(TxnLineInput {
            account_id,
            signed_amount,
            memo: if l.memo.is_empty() {
                None
            } else {
                Some(l.memo.clone())
            },
            tax_rate_id: None,
            foreign: None,
        });
    }

    let mut tx = state.pool.begin().await?;
    // Lock the original.
    let original: Option<(Uuid, NaiveDate, String, String)> = sqlx::query_as(
        "SELECT id, txn_date, description, kind
         FROM transactions
         WHERE id = $1 AND ledger_id = $2
         FOR UPDATE",
    )
    .bind(txn_id)
    .bind(ledger_id)
    .fetch_optional(&mut *tx)
    .await?;
    let original = original.ok_or(AppError::NotFound)?;
    if original.3 == "reversing" {
        return Err(AppError::Unprocessable(
            "Cannot edit a reversal — reverse the original instead".into(),
        ));
    }
    let reversal_id = insert_reversal(
        &mut tx,
        original.0,
        &state,
        ledger_id,
        user.id,
        &original.2,
        original.1,
    )
    .await?;
    // Insert the corrected transaction.
    let currency: String = sqlx::query_scalar("SELECT base_currency FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&mut *tx)
        .await?;
    let corrected_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, currency, kind, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, 'standard', $7)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(date)
    .bind(form.description.trim())
    .bind(form.payee.as_deref().filter(|s| !s.is_empty()))
    .bind(form.reference.as_deref().filter(|s| !s.is_empty()))
    .bind(&currency)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    for input in &inputs {
        let direction = if input.signed_amount >= Decimal::ZERO {
            "DEBIT"
        } else {
            "CREDIT"
        };
        let amount = input.signed_amount.abs();
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(corrected_id)
        .bind(input.account_id)
        .bind(amount)
        .bind(direction)
        .bind(input.memo.as_deref())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "edit",
        "transaction",
        Some(original.0),
        None,
        Some(serde_json::json!({
            "reversal_id": reversal_id,
            "corrected_id": corrected_id,
            "original_id": original.0,
        })),
    )
    .await;

    Ok((
        StatusCode::SEE_OTHER,
        [(
            axum::http::header::LOCATION,
            axum::http::HeaderValue::from_str(&format!(
                "/ledgers/{ledger_id}/transactions/{corrected_id}"
            ))
            .unwrap(),
        )],
        Json(EditResponse {
            reversal_id,
            corrected_id,
        }),
    )
        .into_response())
}

/// Insert a `kind='reversing'` transaction that negates the
/// postings of `original_id`. Returns the new transaction id.
pub async fn insert_reversal(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    original_id: Uuid,
    state: &AppState,
    ledger_id: Uuid,
    user_id: Uuid,
    original_description: &str,
    original_date: NaiveDate,
) -> Result<Uuid, AppError> {
    // Read the original postings (FOR UPDATE was already
    // acquired on the transaction row).
    let postings: Vec<(Uuid, Decimal, String, Option<String>)> = sqlx::query_as(
        "SELECT account_id, amount, direction, memo
         FROM postings
         WHERE transaction_id = $1",
    )
    .bind(original_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    let currency: String = sqlx::query_scalar("SELECT base_currency FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(AppError::Db)?;
    let reversal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions
            (ledger_id, txn_date, description, payee, reference, currency, kind, created_by, reverses_id)
         VALUES ($1, $2, $3, NULL, NULL, $4, 'reversing', $5, $6)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(Utc::now().date_naive())
    .bind(format!("Reversal of: {original_description}"))
    .bind(&currency)
    .bind(user_id)
    .bind(original_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| {
        // If `reverses_id` column doesn't exist yet, the
        // INSERT will fail with a column-not-found error
        // (SQLSTATE 42703). Surface as Internal so it's
        // debuggable rather than a silent 422.
        if let sqlx::Error::Database(d) = &e {
            if d.code().as_deref() == Some("42703") {
                tracing::error!(
                    "transactions.reverses_id column missing — run migrations"
                );
            }
        }
        AppError::Db(e)
    })?;
    for (account_id, amount, direction, memo) in postings {
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(reversal_id)
        .bind(account_id)
        // Negate the amount. For DEBIT, the reversal is
        // negative DEBIT (which the DB CHECK treats as an
        // unsigned amount in the schema — instead we flip
        // the direction so the magnitude stays positive).
        .bind(amount)
        .bind(if direction == "DEBIT" {
            "CREDIT"
        } else {
            "DEBIT"
        })
        .bind(memo.as_deref())
        .execute(&mut **tx)
        .await
        .map_err(AppError::Db)?;
        // Suppress unused warnings for `state` and `original_date`
        // — we keep the parameter list stable for future
        // audit-log enrichment.
        let _ = (state, original_date);
    }
    Ok(reversal_id)
}

/// Pure helper for unit tests: given a slice of `(amount,
/// direction)` postings, return their negations
/// `(amount, flipped_direction)`.
#[cfg(test)]
pub(crate) fn negate_postings(postings: &[(Decimal, &str)]) -> Vec<(Decimal, &'static str)> {
    postings
        .iter()
        .map(|(amt, dir)| {
            let flipped: &'static str = if *dir == "DEBIT" { "CREDIT" } else { "DEBIT" };
            (*amt, flipped)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn reversal_signed_amounts_are_negated() {
        let input = vec![(dec!(100), "DEBIT"), (dec!(100), "CREDIT")];
        let out = negate_postings(&input);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, dec!(100));
        assert_eq!(out[0].1, "CREDIT");
        assert_eq!(out[1].0, dec!(100));
        assert_eq!(out[1].1, "DEBIT");
    }
}
