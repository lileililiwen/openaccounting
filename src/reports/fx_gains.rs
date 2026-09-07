//! Realized & unrealized FX gains report (`multi-currency-fx`).
//!
//! Two non-overlapping sections:
//!
//! - **Unrealized** — every month-end revaluation row
//!   (`fx_revaluations`), i.e. valuation changes on balances still held.
//! - **Realized** — postings to the auto-generated FX P&L accounts from
//!   transactions that are *not* revaluations (e.g. manual FX
//!   adjustment entries settling foreign balances).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UnrealizedRow {
    pub period_month: NaiveDate,
    pub account_name: String,
    pub currency: String,
    pub fx_gain_loss: Decimal,
}

impl UnrealizedRow {
    pub fn is_loss(&self) -> bool {
        self.fx_gain_loss < Decimal::ZERO
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RealizedRow {
    pub txn_date: NaiveDate,
    pub description: String,
    pub gain: Decimal,
    pub loss: Decimal,
}

impl RealizedRow {
    pub fn has_gain(&self) -> bool {
        self.gain != Decimal::ZERO
    }

    pub fn has_loss(&self) -> bool {
        self.loss != Decimal::ZERO
    }
}

#[derive(Debug, Clone, Default)]
pub struct FxGainsReport {
    pub unrealized: Vec<UnrealizedRow>,
    pub realized: Vec<RealizedRow>,
    pub total_unrealized: Decimal,
    pub realized_gain_total: Decimal,
    pub realized_loss_total: Decimal,
}

/// Build the report for a ledger over an optional date range (both
/// bounds inclusive; `None` = unbounded).
pub async fn build_fx_gains(
    pool: &PgPool,
    ledger_id: Uuid,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<FxGainsReport, sqlx::Error> {
    let unrealized: Vec<UnrealizedRow> = sqlx::query_as(
        r#"SELECT r.period_month, a.name AS account_name, a.currency, r.fx_gain_loss
           FROM fx_revaluations r
           JOIN accounts a ON a.id = r.account_id
           WHERE r.ledger_id = $1
                 AND ($2::date IS NULL OR r.period_month >= $2)
                 AND ($3::date IS NULL OR r.period_month <= $3)
           ORDER BY r.period_month, a.name"#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let realized: Vec<RealizedRow> = sqlx::query_as(
        r#"SELECT t.txn_date,
                  t.description,
                  CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE 0 END AS gain,
                  CASE WHEN p.direction = 'CREDIT' THEN p.amount ELSE 0 END AS loss
           FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           JOIN accounts a ON a.id = p.account_id
           WHERE t.ledger_id = $1
                 AND a.name IN ('FX Unrealized Gain/Loss', 'FX Unrealized Loss')
                 AND t.id NOT IN (
                     SELECT transaction_id FROM fx_revaluations WHERE ledger_id = $1)
                 AND ($2::date IS NULL OR t.txn_date >= $2)
                 AND ($3::date IS NULL OR t.txn_date <= $3)
           ORDER BY t.txn_date"#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let total_unrealized = unrealized.iter().map(|r| r.fx_gain_loss).sum();
    let realized_gain_total = realized.iter().map(|r| r.gain).sum();
    let realized_loss_total = realized.iter().map(|r| r.loss).sum();

    Ok(FxGainsReport {
        unrealized,
        realized,
        total_unrealized,
        realized_gain_total,
        realized_loss_total,
    })
}
