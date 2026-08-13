use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::AccountTotal;

#[derive(Clone, Debug)]
pub struct IncomeStatementSection {
    pub label: &'static str,
    pub accounts: Vec<AccountTotal>,
    pub total: Decimal,
}

#[derive(Clone, Debug)]
pub struct IncomeStatementResult {
    pub revenue: IncomeStatementSection,
    pub cost_of_goods_sold: IncomeStatementSection,
    pub gross_profit: Decimal,
    pub operating_expenses: IncomeStatementSection,
    pub operating_income: Decimal,
    pub non_operating: Option<IncomeStatementSection>,
    pub income_before_tax: Decimal,
    pub tax_expense: Option<IncomeStatementSection>,
    pub net_income: Decimal,
}

pub async fn build_income_statement(
    pool: &PgPool,
    ledger_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<IncomeStatementResult> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, String, Decimal)>(
        r#"
        SELECT a.id, a.name, a.type, a.subtype,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw_net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date BETWEEN $2 AND $3
        WHERE a.ledger_id = $1
          AND a.type IN ('INCOME','EXPENSE')
        GROUP BY a.id, a.name, a.type, a.subtype
        ORDER BY a.type, a.subtype, a.name
        "#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    // Build sections from rows
    let make_accounts = |type_filter: &str, subtype_filter: &str| -> Vec<AccountTotal> {
        rows.iter()
            .filter(|r| r.2 == type_filter && r.3 == subtype_filter)
            .map(|r| {
                let amount = if r.2 == "INCOME" { -r.4 } else { r.4 };
                AccountTotal {
                    account_id: r.0,
                    account_name: r.1.clone(),
                    account_type: r.3.clone(),
                    amount,
                }
            })
            .filter(|a| a.amount != Decimal::ZERO)
            .collect()
    };

    // Revenue (OPERATING_INCOME)
    let revenue_accounts = make_accounts("INCOME", "OPERATING_INCOME");
    let revenue_total: Decimal = revenue_accounts.iter().map(|a| a.amount).sum();

    // Cost of Goods Sold
    let cogs_accounts = make_accounts("EXPENSE", "COST_OF_GOODS_SOLD");
    let cogs_total: Decimal = cogs_accounts.iter().map(|a| a.amount).sum();

    let gross_profit = revenue_total - cogs_total;

    // Operating Expenses
    let opex_accounts = make_accounts("EXPENSE", "OPERATING_EXPENSE");
    let opex_total: Decimal = opex_accounts.iter().map(|a| a.amount).sum();

    let operating_income = gross_profit - opex_total;

    // Non-Operating Income/Expense (net)
    let non_op_income_accounts = make_accounts("INCOME", "NON_OPERATING_INCOME");
    let non_op_expense_accounts = make_accounts("EXPENSE", "NON_OPERATING_EXPENSE");
    let non_op_income_total: Decimal = non_op_income_accounts.iter().map(|a| a.amount).sum();
    let non_op_expense_total: Decimal = non_op_expense_accounts.iter().map(|a| a.amount).sum();
    let non_op_net = non_op_income_total - non_op_expense_total;

    let non_operating = if non_op_net != Decimal::ZERO || !non_op_income_accounts.is_empty() || !non_op_expense_accounts.is_empty() {
        let mut accounts = non_op_income_accounts;
        accounts.extend(non_op_expense_accounts);
        Some(IncomeStatementSection {
            label: "Non-Operating Income (Expense)",
            accounts,
            total: non_op_net,
        })
    } else {
        None
    };

    let income_before_tax = operating_income + non_op_net;

    // Tax Expense
    let tax_accounts = make_accounts("EXPENSE", "TAX_EXPENSE");
    let tax_total: Decimal = tax_accounts.iter().map(|a| a.amount).sum();

    let tax_expense = if tax_total != Decimal::ZERO {
        Some(IncomeStatementSection {
            label: "Income Tax Expense",
            accounts: tax_accounts,
            total: tax_total,
        })
    } else {
        None
    };

    let net_income = income_before_tax - tax_total;

    Ok(IncomeStatementResult {
        revenue: IncomeStatementSection {
            label: "Revenue",
            accounts: revenue_accounts,
            total: revenue_total,
        },
        cost_of_goods_sold: IncomeStatementSection {
            label: "Cost of Goods Sold",
            accounts: cogs_accounts,
            total: cogs_total,
        },
        gross_profit,
        operating_expenses: IncomeStatementSection {
            label: "Operating Expenses",
            accounts: opex_accounts,
            total: opex_total,
        },
        operating_income,
        non_operating,
        income_before_tax,
        tax_expense,
        net_income,
    })
}
