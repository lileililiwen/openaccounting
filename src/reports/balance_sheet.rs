use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::AccountTotal;

#[derive(Clone, Debug)]
pub struct BalanceSheetResult {
    pub assets: Vec<AccountTotal>,
    pub liabilities: Vec<AccountTotal>,
    pub equity: Vec<AccountTotal>,
    pub net_income: Decimal,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub total_equity: Decimal,
    pub total_liab_equity: Decimal,
}

pub async fn build_balance_sheet(
    pool: &PgPool,
    ledger_id: Uuid,
    as_of: NaiveDate,
    net_income: Decimal,
) -> AppResult<BalanceSheetResult> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, Decimal)>(
        r#"
        SELECT a.id, a.name, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw_net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date <= $2
        WHERE a.ledger_id = $1
          AND a.type IN ('ASSET','LIABILITY','EQUITY')
        GROUP BY a.id, a.name, a.type
        ORDER BY a.type, a.name
        "#,
    )
    .bind(ledger_id)
    .bind(as_of)
    .fetch_all(pool)
    .await?;

    let mut assets: Vec<AccountTotal> = Vec::new();
    let mut liabilities: Vec<AccountTotal> = Vec::new();
    let mut equity: Vec<AccountTotal> = Vec::new();
    let mut total_assets = Decimal::ZERO;
    let mut total_liab = Decimal::ZERO;
    let mut total_eq = Decimal::ZERO;

    for (id, name, ty, raw) in rows {
        let amount = match ty.as_str() {
            "ASSET" => raw,      // debits - credits
            "LIABILITY" => -raw, // credits - debits
            "EQUITY" => -raw,    // credits - debits
            _ => Decimal::ZERO,
        };
        if amount == Decimal::ZERO {
            continue;
        }
        match ty.as_str() {
            "ASSET" => total_assets += amount,
            "LIABILITY" => total_liab += amount,
            "EQUITY" => total_eq += amount,
            _ => {}
        }
        let vec = match ty.as_str() {
            "ASSET" => &mut assets,
            "LIABILITY" => &mut liabilities,
            "EQUITY" => &mut equity,
            _ => continue,
        };
        vec.push(AccountTotal {
            account_id: id,
            account_name: name,
            account_type: ty,
            amount,
        });
    }
    Ok(BalanceSheetResult {
        assets,
        liabilities,
        equity,
        net_income,
        total_assets,
        total_liabilities: total_liab,
        total_equity: total_eq,
        total_liab_equity: total_liab + total_eq + net_income,
    })
}
