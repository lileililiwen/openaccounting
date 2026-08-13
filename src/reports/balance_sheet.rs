use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::AccountTotal;

#[derive(Clone, Debug)]
pub struct BalanceSheetSection {
    pub label: &'static str,
    pub accounts: Vec<AccountTotal>,
    pub total: Decimal,
}

#[derive(Clone, Debug)]
pub struct BalanceSheetResult {
    pub assets: Vec<BalanceSheetSection>,
    pub liabilities: Vec<BalanceSheetSection>,
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
    let rows = sqlx::query_as::<_, (Uuid, String, String, String, Decimal)>(
        r#"
        SELECT a.id, a.name, a.type, a.subtype,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw_net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date <= $2
        WHERE a.ledger_id = $1
          AND a.type IN ('ASSET','LIABILITY','EQUITY')
        GROUP BY a.id, a.name, a.type, a.subtype
        ORDER BY a.type, a.subtype, a.name
        "#,
    )
    .bind(ledger_id)
    .bind(as_of)
    .fetch_all(pool)
    .await?;

    let mut asset_sections: Vec<BalanceSheetSection> = Vec::new();
    let mut liability_sections: Vec<BalanceSheetSection> = Vec::new();
    let mut equity = Vec::new();
    let mut total_assets = Decimal::ZERO;
    let mut total_liab = Decimal::ZERO;
    let mut total_eq = Decimal::ZERO;

    // Group assets by subtype
    let asset_subtypes = [
        ("CURRENT_ASSET", "Current Assets"),
        ("FIXED_ASSET", "Fixed Assets"),
        ("INTANGIBLE_ASSET", "Intangible Assets"),
        ("OTHER_ASSET", "Other Assets"),
    ];
    for (subtype_key, label) in asset_subtypes {
        let accounts: Vec<AccountTotal> = rows
            .iter()
            .filter(|r| r.2 == "ASSET" && r.3 == subtype_key)
            .map(|r| {
                let amount = r.4; // debits - credits for assets
                AccountTotal {
                    account_id: r.0,
                    account_name: r.1.clone(),
                    account_type: r.2.clone(),
                    amount,
                }
            })
            .filter(|a| a.amount != Decimal::ZERO)
            .collect();
        let total: Decimal = accounts.iter().map(|a| a.amount).sum();
        if !accounts.is_empty() {
            total_assets += total;
            asset_sections.push(BalanceSheetSection {
                label,
                accounts,
                total,
            });
        }
    }

    // Group liabilities by subtype
    let liability_subtypes = [
        ("CURRENT_LIABILITY", "Current Liabilities"),
        ("LONG_TERM_LIABILITY", "Long-Term Liabilities"),
    ];
    for (subtype_key, label) in liability_subtypes {
        let accounts: Vec<AccountTotal> = rows
            .iter()
            .filter(|r| r.2 == "LIABILITY" && r.3 == subtype_key)
            .map(|r| {
                let amount = -r.4; // credits - debits for liabilities
                AccountTotal {
                    account_id: r.0,
                    account_name: r.1.clone(),
                    account_type: r.2.clone(),
                    amount,
                }
            })
            .filter(|a| a.amount != Decimal::ZERO)
            .collect();
        let total: Decimal = accounts.iter().map(|a| a.amount).sum();
        if !accounts.is_empty() {
            total_liab += total;
            liability_sections.push(BalanceSheetSection {
                label,
                accounts,
                total,
            });
        }
    }

    // Equity accounts
    for (id, name, _ty, subtype, raw) in &rows {
        if _ty == "EQUITY" {
            let amount = -raw; // credits - debits
            if amount != Decimal::ZERO {
                total_eq += amount;
                equity.push(AccountTotal {
                    account_id: *id,
                    account_name: name.clone(),
                    account_type: subtype.clone(),
                    amount,
                });
            }
        }
    }

    Ok(BalanceSheetResult {
        assets: asset_sections,
        liabilities: liability_sections,
        equity,
        net_income,
        total_assets,
        total_liabilities: total_liab,
        total_equity: total_eq,
        total_liab_equity: total_liab + total_eq + net_income,
    })
}
