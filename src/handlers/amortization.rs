//! Amortization schedule HTTP handlers (`a10-amortization`).
//!
//! Three actions, all owner- or editor-only:
//! - `GET /ledgers/{id}/amortization/new` — form.
//! - `POST /ledgers/{id}/amortization/new` — create a schedule.
//! - `POST /ledgers/{id}/amortization/{sid}/skip` — mark the
//!   next period as skipped.
//! - `GET /ledgers/{id}/reports/amortization` — read-only list.

use crate::error::{AppError, AppResult};
use crate::templates::amortization::{
    AmortizationNewPage, AmortizationReportPage, AmortizationRowTemplate,
};
use crate::templates::render_response;
use crate::AppState;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::Backend;
use crate::handlers::ledgers;

pub async fn list_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let schedules = crate::reports::amortization::list_schedules(&state.pool, ledger_id).await?;
    let rows: Vec<AmortizationRowTemplate> = schedules
        .into_iter()
        .map(|r| AmortizationRowTemplate {
            schedule_id: r.schedule_id,
            description: r.description,
            source_account_name: r.source_account_name,
            target_account_name: r.target_account_name,
            total_amount: r.total_amount,
            posted_amount: r.posted_amount,
            remaining_amount: r.remaining_amount,
            period_unit: r.period_unit,
            periods: r.periods,
            posted_periods: r.posted_periods,
            skipped_periods: r.skipped_periods,
            start_date: r.start_date,
            end_date: r.end_date,
            next_post_date: r.next_post_date,
        })
        .collect();
    Ok(render_response(AmortizationReportPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "reports".to_string(),
        schedules: rows,
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, code, name FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE
         ORDER BY type, code NULLS LAST, name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(AmortizationNewPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "reports".to_string(),
        accounts,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewAmortizationForm {
    pub description: String,
    pub source_account_id: Uuid,
    pub target_account_id: Uuid,
    pub total_amount: Decimal,
    pub period_unit: String,
    pub periods: i32,
    pub start_date: NaiveDate,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewAmortizationForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    if !matches!(
        form.period_unit.as_str(),
        "monthly" | "quarterly" | "yearly"
    ) {
        return Err(AppError::Validation(format!(
            "invalid period_unit: {}",
            form.period_unit
        )));
    }
    if form.periods <= 0 {
        return Err(AppError::Validation("periods must be > 0".into()));
    }
    if form.total_amount < Decimal::ZERO {
        return Err(AppError::Validation("total_amount must be >= 0".into()));
    }
    if form.source_account_id == form.target_account_id {
        return Err(AppError::Validation("source and target must differ".into()));
    }
    let end = advance(form.start_date, &form.period_unit, form.periods - 1);
    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO amortization_schedules
            (ledger_id, description, source_account_id, target_account_id,
             total_amount, period_unit, periods, start_date, end_date,
             next_post_date)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(form.description.trim())
    .bind(form.source_account_id)
    .bind(form.target_account_id)
    .bind(form.total_amount)
    .bind(&form.period_unit)
    .bind(form.periods)
    .bind(form.start_date)
    .bind(end)
    .bind(form.start_date)
    .fetch_one(&state.pool)
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/reports/amortization")).into_response())
}

#[derive(Deserialize)]
pub struct SkipForm {
    #[serde(default)]
    pub _placeholder: Option<String>,
}

pub async fn skip(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, schedule_id)): Path<(Uuid, Uuid)>,
    Form(_form): Form<SkipForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    sqlx::query(
        "UPDATE amortization_schedules
         SET skipped_periods = skipped_periods + 1,
             next_post_date = next_post_date + interval '1 month',
             updated_at = now()
         WHERE id = $1 AND ledger_id = $2 AND is_active = TRUE",
    )
    .bind(schedule_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/reports/amortization")).into_response())
}

/// Date math matching the worker.
fn advance(date: NaiveDate, unit: &str, n: i32) -> NaiveDate {
    use chrono::Datelike;
    let n = n as i64;
    match unit {
        "monthly" => {
            let mut y = date.year() as i64;
            let mut m = date.month() as i64 + n;
            while m > 12 {
                m -= 12;
                y += 1;
            }
            while m < 1 {
                m += 12;
                y -= 1;
            }
            NaiveDate::from_ymd_opt(y as i32, m as u32, date.day()).unwrap_or(date)
        }
        "quarterly" => {
            let mut y = date.year() as i64;
            let mut m = date.month() as i64 + 3 * n;
            while m > 12 {
                m -= 12;
                y += 1;
            }
            while m < 1 {
                m += 12;
                y -= 1;
            }
            NaiveDate::from_ymd_opt(y as i32, m as u32, date.day()).unwrap_or(date)
        }
        "yearly" => NaiveDate::from_ymd_opt(date.year() + n as i32, date.month(), date.day())
            .unwrap_or(date),
        _ => date,
    }
}
