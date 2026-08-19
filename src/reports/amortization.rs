//! Amortization schedule report (`a10-amortization`).
//!
//! Read-only: lists every active schedule with
//! posted / remaining / next-date / total / description. Used
//! by the `/reports/amortization` page.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Clone, Debug)]
pub struct AmortizationRow {
    pub schedule_id: Uuid,
    pub description: String,
    pub source_account_name: String,
    pub target_account_name: String,
    pub total_amount: Decimal,
    pub posted_amount: Decimal,
    pub remaining_amount: Decimal,
    pub period_unit: String,
    pub periods: i32,
    pub posted_periods: i32,
    pub skipped_periods: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub next_post_date: NaiveDate,
}

pub async fn list_schedules(pool: &PgPool, ledger_id: Uuid) -> AppResult<Vec<AmortizationRow>> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            Decimal,
            String,
            i32,
            i32,
            i32,
            NaiveDate,
            NaiveDate,
            NaiveDate,
        ),
    >(
        r#"SELECT s.id, s.description,
                  sa.name, ta.name,
                  s.total_amount,
                  s.period_unit,
                  s.periods,
                  s.posted_periods,
                  s.skipped_periods,
                  s.start_date, s.end_date, s.next_post_date
           FROM amortization_schedules s
           JOIN accounts sa ON sa.id = s.source_account_id
           JOIN accounts ta ON ta.id = s.target_account_id
           WHERE s.ledger_id = $1
           ORDER BY s.is_active DESC, s.next_post_date, s.created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;

    let out = rows
        .into_iter()
        .map(
            |(
                schedule_id,
                description,
                source_account_name,
                target_account_name,
                total_amount,
                period_unit,
                periods,
                posted_periods,
                skipped_periods,
                start_date,
                end_date,
                next_post_date,
            )| {
                let per_period = if periods > 0 {
                    total_amount / Decimal::from(periods)
                } else {
                    Decimal::ZERO
                };
                let posted_amount = per_period * Decimal::from(posted_periods);
                let remaining_amount = total_amount - posted_amount;
                AmortizationRow {
                    schedule_id,
                    description,
                    source_account_name,
                    target_account_name,
                    total_amount,
                    posted_amount,
                    remaining_amount,
                    period_unit,
                    periods,
                    posted_periods,
                    skipped_periods,
                    start_date,
                    end_date,
                    next_post_date,
                }
            },
        )
        .collect();
    Ok(out)
}
