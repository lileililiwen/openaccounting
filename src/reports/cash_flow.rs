use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::ReportBasis;

#[derive(Clone, Debug)]
pub struct CashFlowLine {
    pub account_name: String,
    pub amount: Decimal,
}

#[derive(Clone, Debug)]
pub struct CashFlowResult {
    pub basis: ReportBasis,
    pub opening: Decimal,
    pub closing: Decimal,
    pub movement: Decimal,
    pub inflows: Vec<CashFlowLine>,
    pub outflows: Vec<CashFlowLine>,
    pub total_inflows: Decimal,
    pub total_outflows: Decimal,
}

/// Cash flow: focused on ASSET accounts whose name suggests "cash" (heuristic:
/// code starts with 1, name contains 'cash' or 'bank'). Opening is balance at
/// `from`; closing is balance at `to`; movement = closing - opening.
///
/// The `basis` argument is accepted for symmetry with the income
/// statement: the report's totals are identical under both bases
/// because the cash-account filter is intrinsic to the report.
/// The value is recorded in the result so the page footer can be
/// honest about which basis the user requested.
pub async fn build_cash_flow(
    pool: &PgPool,
    ledger_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    basis: ReportBasis,
) -> AppResult<CashFlowResult> {
    // Identify cash accounts in this ledger.
    let cash_ids: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM accounts
        WHERE ledger_id = $1
          AND type = 'ASSET'
          AND (LOWER(name) LIKE '%cash%' OR LOWER(name) LIKE '%bank%' OR code LIKE '1%')
        "#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;

    if cash_ids.is_empty() {
        return Ok(CashFlowResult {
            basis,
            opening: Decimal::ZERO,
            closing: Decimal::ZERO,
            movement: Decimal::ZERO,
            inflows: vec![],
            outflows: vec![],
            total_inflows: Decimal::ZERO,
            total_outflows: Decimal::ZERO,
        });
    }

    let ids: Vec<Uuid> = cash_ids.iter().map(|(id,)| *id).collect();

    // Opening and closing balances across all cash accounts.
    let bal_query = format!(
        r#"
        SELECT
          COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
        - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS bal
        FROM postings p
        JOIN transactions t ON t.id = p.transaction_id
        WHERE t.ledger_id = $1
          AND p.account_id = ANY($2)
          AND t.txn_date <= $3
        "#
    );

    let opening: (Decimal,) = sqlx::query_as(&bal_query)
        .bind(ledger_id)
        .bind(&ids)
        .bind(from)
        .fetch_one(pool)
        .await?;
    let closing: (Decimal,) = sqlx::query_as(&bal_query)
        .bind(ledger_id)
        .bind(&ids)
        .bind(to)
        .fetch_one(pool)
        .await?;

    // Inflows/outflows grouped by account within the period.
    let flow_rows = sqlx::query_as::<_, (Uuid, String, Decimal)>(
        r#"
        SELECT a.id, a.name,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw
        FROM accounts a
        JOIN postings p ON p.account_id = a.id
        JOIN transactions t ON t.id = p.transaction_id
        WHERE t.ledger_id = $1
          AND t.txn_date BETWEEN $2 AND $3
          AND a.id = ANY($4)
        GROUP BY a.id, a.name
        ORDER BY a.name
        "#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .bind(&ids)
    .fetch_all(pool)
    .await?;

    let mut inflows = Vec::new();
    let mut outflows = Vec::new();
    let mut total_in = Decimal::ZERO;
    let mut total_out = Decimal::ZERO;
    for (_id, name, raw) in flow_rows {
        if raw > Decimal::ZERO {
            total_in += raw;
            inflows.push(CashFlowLine {
                account_name: name,
                amount: raw,
            });
        } else if raw < Decimal::ZERO {
            total_out += -raw;
            outflows.push(CashFlowLine {
                account_name: name,
                amount: -raw,
            });
        }
    }

    Ok(CashFlowResult {
        basis,
        opening: opening.0,
        closing: closing.0,
        movement: closing.0 - opening.0,
        inflows,
        outflows,
        total_inflows: total_in,
        total_outflows: total_out,
    })
}
