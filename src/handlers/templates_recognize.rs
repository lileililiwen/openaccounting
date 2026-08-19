//! Revenue / expense recognition helpers (`a9-cash-basis-docs`).
//!
//! The existing `transaction_templates` infrastructure is reused:
//! a recognition schedule is just a template with two postings
//! (debit DeferredRevenue, credit Revenue — or vice-versa for
//! PrepaidExpense) and a recurring frequency. When the recurring
//! worker fires it, a balanced transaction is created.
//!
//! These handlers expose:
//! - `POST /ledgers/{id}/recognize/revenue` — create a monthly
//!   recognition schedule from a DeferredRevenue balance to a
//!   Revenue account.
//! - `POST /ledgers/{id}/recognize/expense` — symmetric action
//!   from PrepaidExpense to an Expense account.
//!
//! Both default to monthly recognition starting today. The amount
//! is the user's input; the spec requires one balanced txn per
//! period, so the templates carry a single fixed amount.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    Form, Router,
};
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/ledgers/{ledger_id}/recognize/revenue",
            axum::routing::post(create_revenue_recognition),
        )
        .route(
            "/ledgers/{ledger_id}/recognize/expense",
            axum::routing::post(create_expense_recognition),
        )
}

#[derive(Deserialize, Debug)]
pub struct RecognizeForm {
    pub description: String,
    /// Source account: DeferredRevenue (revenue path) or
    /// PrepaidExpense (expense path).
    pub source_account_id: Uuid,
    /// Target account: Revenue (revenue path) or Expense
    /// (expense path).
    pub target_account_id: Uuid,
    /// Amount to recognize per period.
    pub amount: Decimal,
    /// First day of recognition (defaults to today when absent).
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
    /// "monthly" (default) or "quarterly" or "yearly".
    #[serde(default = "default_frequency")]
    pub frequency: String,
    /// "monthly" only: how many periods to recognize for. When
    /// `None` the schedule runs until the user deactivates it
    /// or the source balance is exhausted.
    #[serde(default)]
    pub periods: Option<i32>,
}

fn default_frequency() -> String {
    "monthly".to_string()
}

pub async fn create_revenue_recognition(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<RecognizeForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let template_id = create_recognition_template(
        &state, ledger_id, user.id, &form, /* source_direction = */ "DEBIT",
        /* target_direction = */ "CREDIT",
    )
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/templates/{template_id}")).into_response())
}

pub async fn create_expense_recognition(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<RecognizeForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let template_id = create_recognition_template(
        &state, ledger_id, user.id, &form, /* source_direction = */ "CREDIT",
        /* target_direction = */ "DEBIT",
    )
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/templates/{template_id}")).into_response())
}

async fn create_recognition_template(
    state: &AppState,
    ledger_id: Uuid,
    user_id: Uuid,
    form: &RecognizeForm,
    source_direction: &str,
    target_direction: &str,
) -> AppResult<Uuid> {
    if form.amount <= Decimal::ZERO {
        return Err(AppError::Validation("amount must be positive".into()));
    }
    let frequency = form.frequency.as_str();
    if !["monthly", "quarterly", "yearly"].contains(&frequency) {
        return Err(AppError::Validation(format!(
            "unsupported frequency `{frequency}`"
        )));
    }
    let description = form.description.trim();
    if description.is_empty() {
        return Err(AppError::Validation("description is required".into()));
    }
    if form.source_account_id == form.target_account_id {
        return Err(AppError::Validation(
            "source and target accounts must differ".into(),
        ));
    }
    let start = form
        .start_date
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
    let memo = format!("Recognition #{frequency} from {start}");
    // Append "(N periods)" when the user gave a finite count so
    // the audit log explains the schedule's terminal date.
    let description = match form.periods {
        Some(n) => format!("{description} ({n} periods)"),
        None => description.to_string(),
    };

    let mut tx = state.pool.begin().await?;

    // Verify both accounts exist in this ledger.
    for acct in [form.source_account_id, form.target_account_id] {
        let exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE id = $1 AND ledger_id = $2")
                .bind(acct)
                .bind(ledger_id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(AppError::Validation(format!(
                "account {acct} does not belong to this ledger"
            )));
        }
    }

    let template_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transaction_templates
            (ledger_id, description, payee, reference, frequency, next_date)
           VALUES ($1, $2, NULL, NULL, $3, $4)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(&description)
    .bind(frequency)
    .bind(start)
    .fetch_one(&mut *tx)
    .await?;

    // Two balanced postings: source (one direction) and target
    // (opposite direction), equal magnitude.
    sqlx::query(
        r#"INSERT INTO template_postings
            (template_id, account_id, direction, amount, memo)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(template_id)
    .bind(form.source_account_id)
    .bind(source_direction)
    .bind(form.amount)
    .bind(format!("Recognition: {memo}"))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO template_postings
            (template_id, account_id, direction, amount, memo)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(template_id)
    .bind(form.target_account_id)
    .bind(target_direction)
    .bind(form.amount)
    .bind(format!("Recognition: {memo}"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user_id,
        "create",
        "recognition_template",
        Some(template_id),
        None,
        Some(serde_json::json!({
            "source_account_id": form.source_account_id,
            "target_account_id": form.target_account_id,
            "amount": form.amount.to_string(),
            "frequency": frequency,
            "start_date": start,
            "periods": form.periods,
        })),
    )
    .await;
    Ok(template_id)
}
