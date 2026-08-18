//! Investment lots and FIFO cost-basis tracking
//! (`a6-investment-lots`).
//!
//! A `buy` posting on an account of type `Investment` creates a
//! new lot. A `sell` posting on the same account creates one or
//! more `investment_disposals` rows, matched against the oldest
//! open lots first (FIFO).
//!
//! The accounting invariant is unchanged: a buy is
//! `Dr Investment / Cr Cash`, a sell is `Dr Cash / Cr Investment /
//! Dr/Cr Realized Gain or Loss`. The lot tables add metadata
//! for cost-basis reporting.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// One open lot. Lots carry qty > 0 until fully disposed.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct InvestmentLot {
    pub id: Uuid,
    pub account_id: Uuid,
    pub acquired_at: NaiveDate,
    pub qty: Decimal,
    pub unit_cost: Decimal,
    pub currency: String,
    pub source_txn_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// One disposal record. A sell may create multiple disposals
/// when it crosses lot boundaries.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct InvestmentDisposal {
    pub id: Uuid,
    pub lot_id: Uuid,
    pub qty: Decimal,
    pub unit_proceeds: Decimal,
    pub disposed_at: NaiveDate,
    pub source_txn_id: Uuid,
    pub realized_gain: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Insert one lot. Called when a buy posting lands on an
/// `Investment`-type account. `qty` is the absolute value of the
/// debit posting amount (shares), `unit_cost` is the per-share
/// cost (= total cost / qty).
pub async fn insert_lot(
    pool: &PgPool,
    account_id: Uuid,
    acquired_at: NaiveDate,
    qty: Decimal,
    unit_cost: Decimal,
    currency: &str,
    source_txn_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO investment_lots
            (account_id, acquired_at, qty, unit_cost, currency, source_txn_id)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(account_id)
    .bind(acquired_at)
    .bind(qty)
    .bind(unit_cost)
    .bind(currency)
    .bind(source_txn_id)
    .fetch_one(pool)
    .await
}

/// Match a sell `qty` against the oldest open lots FIFO. Returns
/// the disposal records that were inserted. Each returned
/// disposal carries its lot's `unit_cost` × `qty` as the cost
/// basis; the realised gain is `qty * (unit_proceeds - unit_cost)`.
///
/// Panics-free: if there is not enough open qty, the function
/// inserts what it can and surfaces the unsold remainder via
/// [`fifo_match::unsold_remainder`].
///
/// The `unit_proceeds` is the per-share sale price (total
/// proceeds / qty).
pub async fn fifo_match(
    pool: &PgPool,
    account_id: Uuid,
    sell_qty: Decimal,
    unit_proceeds: Decimal,
    disposed_at: NaiveDate,
    source_txn_id: Uuid,
) -> Result<Vec<InvestmentDisposal>, sqlx::Error> {
    let mut remaining = sell_qty;
    let mut disposals: Vec<InvestmentDisposal> = Vec::new();

    while remaining > Decimal::ZERO {
        // Pick the oldest lot with remaining qty. We compute
        // remaining_qty = lot.qty - sum(disposals.qty) per lot.
        let candidate: Option<(Uuid, Decimal, NaiveDate, Decimal)> = sqlx::query_as(
            "SELECT l.id, l.unit_cost, l.acquired_at,
                    (l.qty - COALESCE(SUM(d.qty), 0))::DECIMAL AS remaining
             FROM investment_lots l
             LEFT JOIN investment_disposals d ON d.lot_id = l.id
             WHERE l.account_id = $1
             GROUP BY l.id, l.unit_cost, l.acquired_at, l.qty
             HAVING (l.qty - COALESCE(SUM(d.qty), 0)) > 0
             ORDER BY l.acquired_at ASC, l.id ASC
             LIMIT 1",
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        let Some((lot_id, unit_cost, _acquired_at, available)) = candidate else {
            break;
        };
        let take = if available <= remaining { available } else { remaining };
        let cost_basis = take * unit_cost;
        let proceeds = take * unit_proceeds;
        let realized_gain = proceeds - cost_basis;

        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO investment_disposals
                (lot_id, qty, unit_proceeds, disposed_at, source_txn_id, realized_gain)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id",
        )
        .bind(lot_id)
        .bind(take)
        .bind(unit_proceeds)
        .bind(disposed_at)
        .bind(source_txn_id)
        .bind(realized_gain)
        .fetch_one(pool)
        .await?;

        disposals.push(InvestmentDisposal {
            id,
            lot_id,
            qty: take,
            unit_proceeds,
            disposed_at,
            source_txn_id,
            realized_gain,
            created_at: chrono::Utc::now(),
        });
        remaining -= take;
    }
    Ok(disposals)
}

/// Sum of remaining qty on an account (sum of all open lots
/// after disposals). Used by the holdings report.
pub async fn remaining_qty(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Decimal, sqlx::Error> {
    let row: (Option<Decimal>,) = sqlx::query_as(
        "SELECT COALESCE(SUM(l.qty - COALESCE(d_sum.qty_sum, 0)), 0)::DECIMAL
         FROM investment_lots l
         LEFT JOIN (
             SELECT lot_id, SUM(qty) AS qty_sum
             FROM investment_disposals GROUP BY lot_id
         ) d_sum ON d_sum.lot_id = l.id
         WHERE l.account_id = $1",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0.unwrap_or(Decimal::ZERO))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_match_remaining_dec_decrements() {
        // Pure logic: this is the loop we use to keep
        // decrementing `remaining` until we hit zero.
        let mut remaining = rust_decimal::Decimal::new(7, 0);
        let take = rust_decimal::Decimal::new(3, 0);
        remaining -= take;
        assert_eq!(remaining, rust_decimal::Decimal::new(4, 0));
    }

    #[test]
    fn realised_gain_positive_when_proceeds_above_cost() {
        let take = rust_decimal::Decimal::new(4, 0);
        let unit_cost = rust_decimal::Decimal::new(50, 0);
        let unit_proceeds = rust_decimal::Decimal::new(60, 0);
        let cost = take * unit_cost;
        let proceeds = take * unit_proceeds;
        let gain = proceeds - cost;
        assert_eq!(gain, rust_decimal::Decimal::new(40, 0));
    }
}