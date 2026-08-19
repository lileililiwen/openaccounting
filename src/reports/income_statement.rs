use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::AccountTotal;
use super::ReportBasis;

#[derive(Clone, Debug)]
pub struct IncomeStatementSection {
    pub label: &'static str,
    pub accounts: Vec<AccountTotal>,
    pub total: Decimal,
}

/// Sums that were excluded from the income statement because of
/// the cash-basis filter. Populated when `basis = Cash`; `None`
/// for the accrual view.
#[derive(Clone, Debug, Default)]
pub struct ExcludedTotals {
    pub revenue: Decimal,
    pub expense: Decimal,
}

#[derive(Clone, Debug)]
pub struct IncomeStatementResult {
    pub basis: ReportBasis,
    pub revenue: IncomeStatementSection,
    pub cost_of_goods_sold: IncomeStatementSection,
    pub gross_profit: Decimal,
    pub operating_expenses: IncomeStatementSection,
    pub operating_income: Decimal,
    pub non_operating: Option<IncomeStatementSection>,
    pub income_before_tax: Decimal,
    pub tax_expense: Option<IncomeStatementSection>,
    pub net_income: Decimal,
    /// Totals excluded by the cash-basis filter (only set when
    /// `basis = Cash`).
    pub excluded: Option<ExcludedTotals>,
}

#[derive(sqlx::FromRow)]
struct IncomeRow {
    id: Uuid,
    name: String,
    type_: String,
    subtype: String,
    /// Net movement under accrual: debits minus credits.
    accrual_raw_net: Decimal,
    /// Net movement under cash: debits minus credits, restricted
    /// to postings whose peer leg is a cash / bank account.
    cash_raw_net: Decimal,
}

/// Build the income statement for a ledger in a date range.
///
/// When `basis = Cash`, postings whose peer leg is NOT a cash /
/// bank account are excluded from the totals. The "excluded"
/// amounts (the difference between accrual and cash) are
/// returned so the report can show a footnote to the user.
pub async fn build_income_statement(
    pool: &PgPool,
    ledger_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    basis: ReportBasis,
) -> AppResult<IncomeStatementResult> {
    // One round-trip computes both totals: every row carries
    // its accrual net movement AND its cash-basis net movement
    // (the latter is zero when the peer leg is not a cash
    // account). The Rust layer then picks the active basis and
    // records the excluded amounts when `basis = Cash`.
    let rows: Vec<IncomeRow> = sqlx::query_as(
        r#"
        WITH peer AS (
            SELECT p.id AS posting_id,
                   p.transaction_id,
                   p.account_id AS own_account_id,
                   EXISTS (
                       SELECT 1 FROM postings p2
                       JOIN accounts a2 ON a2.id = p2.account_id
                       WHERE p2.transaction_id = p.transaction_id
                         AND p2.id <> p.id
                         AND (LOWER(a2.name) LIKE '%cash%' OR LOWER(a2.name) LIKE '%bank%')
                   ) AS peer_is_cash
            FROM postings p
        )
        SELECT a.id, a.name, a.type AS type_, a.subtype,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS accrual_raw_net,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  AND peer.peer_is_cash THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' AND peer.peer_is_cash THEN p.amount ELSE 0 END), 0) AS cash_raw_net
        FROM accounts a
        JOIN postings p ON p.account_id = a.id
        JOIN transactions t ON t.id = p.transaction_id AND t.txn_date BETWEEN $2 AND $3 AND t.kind != 'draft'
        JOIN peer ON peer.posting_id = p.id
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

    let raw_net_for_basis = |row: &IncomeRow| -> Decimal {
        match basis {
            ReportBasis::Accrual => row.accrual_raw_net,
            ReportBasis::Cash => row.cash_raw_net,
        }
    };

    // Build sections from rows. The "sign flip" for INCOME
    // accounts (their normal balance is credit) is applied to
    // both the active-basis amount and the accrual amount so the
    // excluded footer has consistent units.
    let make_accounts = |type_filter: &str, subtype_filter: &str| -> Vec<AccountTotal> {
        rows.iter()
            .filter(|r| r.type_ == type_filter && r.subtype == subtype_filter)
            .map(|r| {
                let raw = raw_net_for_basis(r);
                let amount = if r.type_ == "INCOME" { -raw } else { raw };
                AccountTotal {
                    account_id: r.id,
                    account_name: r.name.clone(),
                    account_type: r.subtype.clone(),
                    amount,
                }
            })
            .filter(|a| a.amount != Decimal::ZERO)
            .collect()
    };

    let revenue_accounts = make_accounts("INCOME", "OPERATING_INCOME");
    let revenue_total: Decimal = revenue_accounts.iter().map(|a| a.amount).sum();

    let cogs_accounts = make_accounts("EXPENSE", "COST_OF_GOODS_SOLD");
    let cogs_total: Decimal = cogs_accounts.iter().map(|a| a.amount).sum();

    let gross_profit = revenue_total - cogs_total;

    let opex_accounts = make_accounts("EXPENSE", "OPERATING_EXPENSE");
    let opex_total: Decimal = opex_accounts.iter().map(|a| a.amount).sum();

    let operating_income = gross_profit - opex_total;

    let non_op_income_accounts = make_accounts("INCOME", "NON_OPERATING_INCOME");
    let non_op_expense_accounts = make_accounts("EXPENSE", "NON_OPERATING_EXPENSE");
    let non_op_income_total: Decimal = non_op_income_accounts.iter().map(|a| a.amount).sum();
    let non_op_expense_total: Decimal = non_op_expense_accounts.iter().map(|a| a.amount).sum();
    let non_op_net = non_op_income_total - non_op_expense_total;

    let non_operating = if non_op_net != Decimal::ZERO
        || !non_op_income_accounts.is_empty()
        || !non_op_expense_accounts.is_empty()
    {
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

    // Compute the excluded footer when the cash basis is in
    // effect: excluded = accrual totals - cash totals. Both
    // sides are sign-flipped consistently with the section
    // amounts above.
    let excluded = if basis == ReportBasis::Cash {
        let accrual_revenue: Decimal = rows
            .iter()
            .filter(|r| r.type_ == "INCOME" && r.subtype == "OPERATING_INCOME")
            .map(|r| -r.accrual_raw_net)
            .sum();
        let accrual_expense: Decimal = rows
            .iter()
            .filter(|r| r.type_ == "EXPENSE")
            .map(|r| r.accrual_raw_net)
            .sum();
        Some(ExcludedTotals {
            revenue: accrual_revenue - revenue_total,
            expense: accrual_expense - (cogs_total + opex_total + tax_total),
        })
    } else {
        None
    };

    Ok(IncomeStatementResult {
        basis,
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
        excluded,
    })
}
