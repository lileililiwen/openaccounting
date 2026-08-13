use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::AccountTotal;

#[derive(Clone, Debug)]
pub struct IncomeStatementResult {
    pub income: Vec<AccountTotal>,
    pub expense: Vec<AccountTotal>,
    pub total_income: Decimal,
    pub total_expense: Decimal,
    pub net_income: Decimal,
}

pub async fn build_income_statement(
    pool: &PgPool,
    ledger_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<IncomeStatementResult> {
    // For each income/expense account: net movement within the period.
    //   INCOME (credit-normal): net = credits - debits
    //   EXPENSE (debit-normal): net = debits - credits
    let rows = sqlx::query_as::<_, (Uuid, String, String, Decimal)>(
        r#"
        SELECT a.id, a.name, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw_net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date BETWEEN $2 AND $3
        WHERE a.ledger_id = $1
          AND a.type IN ('INCOME','EXPENSE')
        GROUP BY a.id, a.name, a.type
        ORDER BY a.type, a.name
        "#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let mut income = Vec::new();
    let mut expense = Vec::new();
    let mut total_income = Decimal::ZERO;
    let mut total_expense = Decimal::ZERO;

    for (id, name, ty, raw) in rows {
        let amount = match ty.as_str() {
            "INCOME" => -raw, // credits - debits
            "EXPENSE" => raw, // debits - credits
            _ => Decimal::ZERO,
        };
        if amount == Decimal::ZERO {
            continue;
        }
        match ty.as_str() {
            "INCOME" => total_income += amount,
            "EXPENSE" => total_expense += amount,
            _ => {}
        }
        let vec = if ty == "INCOME" {
            &mut income
        } else {
            &mut expense
        };
        vec.push(AccountTotal {
            account_id: id,
            account_name: name,
            account_type: ty,
            amount,
        });
    }
    Ok(IncomeStatementResult {
        income,
        expense,
        total_income,
        total_expense,
        net_income: total_income - total_expense,
    })
}
