use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::dimensions::DimensionFilter;
use crate::error::AppResult;

#[derive(Clone, Debug)]
pub struct TrialBalanceRow {
    pub account_id: Uuid,
    pub account_name: String,
    pub account_type: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

#[derive(Clone, Debug, Default)]
pub struct TrialBalanceResult {
    pub rows: Vec<TrialBalanceRow>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    /// Postings in scope whose filtered dimension is NULL, aggregated
    /// as the Unassigned bucket (`accounting-dimensions`). `None` when
    /// no dimension filter is active.
    pub unassigned: Option<(Decimal, Decimal)>,
}

pub async fn build_trial_balance(
    pool: &PgPool,
    ledger_id: Uuid,
    as_of: NaiveDate,
) -> AppResult<TrialBalanceResult> {
    build_trial_balance_filtered(pool, ledger_id, as_of, &DimensionFilter::empty()).await
}

/// Dimension-sliced trial balance. The filter restricts postings to one
/// cost center / project; untagged postings aggregate under Unassigned.
pub async fn build_trial_balance_filtered(
    pool: &PgPool,
    ledger_id: Uuid,
    as_of: NaiveDate,
    filter: &DimensionFilter,
) -> AppResult<TrialBalanceResult> {
    // Per-account net movement, as_of. For debit-normal accounts the natural
    // side is DEBIT; for credit-normal the natural side is CREDIT. Show only
    // the non-zero column so the report stays readable.
    let rows = sqlx::query_as::<_, (Uuid, String, String, Decimal)>(
        r#"
        SELECT a.id, a.name, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT' THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
            AND ($3 IS NULL OR p.cost_center_id = $3)
            AND ($4 IS NULL OR p.project_id = $4)
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date <= $2 AND t.kind != 'draft'
        WHERE a.ledger_id = $1
        GROUP BY a.id, a.name, a.type
        ORDER BY a.type, a.name
        "#,
    )
    .bind(ledger_id)
    .bind(as_of)
    .bind(filter.cost_center_id)
    .bind(filter.project_id)
    .fetch_all(pool)
    .await?;

    let mut out_rows = Vec::new();
    let mut total_dr = Decimal::ZERO;
    let mut total_cr = Decimal::ZERO;
    for (id, name, ty, net) in rows {
        if net == Decimal::ZERO {
            continue;
        }
        // net = debits - credits (per account)
        // For a debit-normal account (ASSET, EXPENSE) a positive net means
        // the account increased; that lands in the DEBIT column.
        // For a credit-normal account (LIABILITY, EQUITY, INCOME) a positive
        // net means credits exceeded debits, so the amount lands in the
        // CREDIT column.
        let (dr, cr) = match ty.as_str() {
            "ASSET" | "EXPENSE" => (net.max(Decimal::ZERO), Decimal::ZERO),
            _ => (Decimal::ZERO, (-net).max(Decimal::ZERO)),
        };
        total_dr += dr;
        total_cr += cr;
        out_rows.push(TrialBalanceRow {
            account_id: id,
            account_name: name,
            account_type: ty,
            debit: dr,
            credit: cr,
        });
    }
    Ok(TrialBalanceResult {
        rows: out_rows,
        total_debit: total_dr,
        total_credit: total_cr,
        unassigned: if filter.is_active() {
            Some(unassigned_sums(pool, ledger_id, as_of, filter).await?)
        } else {
            None
        },
    })
}

/// Debit/credit sums of in-scope postings whose filtered dimension is
/// NULL — the Unassigned bucket shown alongside a dimension slice.
async fn unassigned_sums(
    pool: &PgPool,
    ledger_id: Uuid,
    as_of: NaiveDate,
    filter: &DimensionFilter,
) -> AppResult<(Decimal, Decimal)> {
    // Untagged on any *filtered* dimension: when slicing by cost
    // center C, postings with no cost center are Unassigned (their
    // project value is irrelevant, and vice versa).
    let row: (Decimal, Decimal) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(CASE WHEN p.direction='DEBIT' THEN p.amount ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0)
        FROM postings p
        JOIN transactions t ON t.id = p.transaction_id AND t.txn_date <= $2 AND t.kind != 'draft'
        JOIN accounts a ON a.id = p.account_id AND a.ledger_id = $1
        WHERE ($3 AND p.cost_center_id IS NULL)
           OR ($4 AND p.project_id IS NULL)
        "#,
    )
    .bind(ledger_id)
    .bind(as_of)
    .bind(filter.cost_center_id.is_some())
    .bind(filter.project_id.is_some())
    .fetch_one(pool)
    .await?;
    Ok(row)
}
