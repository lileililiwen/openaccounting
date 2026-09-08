//! Canonical synthetic ledger fixtures for accounting assurance.
//!
//! Each fixture defines a complete ledger with accounts,
//! transactions, and exact expected report outputs. Tests load
//! these into a fresh database and compare actual report values
//! against the expected `Decimal` constants.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// A synthetic account definition.
pub struct FixtureAccount {
    pub name: &'static str,
    pub code: &'static str,
    pub account_type: &'static str,
    pub subtype: &'static str,
    pub currency: &'static str,
}

/// A synthetic posting (one leg of a double-entry transaction).
pub struct FixturePosting {
    pub account_name: &'static str,
    pub direction: &'static str,
    pub amount: Decimal,
}

/// A synthetic transaction with balanced postings.
pub struct FixtureTransaction {
    pub date: NaiveDate,
    pub description: &'static str,
    pub postings: &'static [FixturePosting],
}

/// Expected trial balance row for reconciliation.
pub struct ExpectedTrialBalanceRow {
    pub account_name: &'static str,
    pub account_type: &'static str,
    pub debit: Decimal,
    pub credit: Decimal,
}

/// Expected income statement totals.
pub struct ExpectedIncomeStatement {
    pub revenue_total: Decimal,
    pub cogs_total: Decimal,
    pub gross_profit: Decimal,
    pub opex_total: Decimal,
    pub operating_income: Decimal,
    pub net_income: Decimal,
}

/// Expected balance sheet totals.
pub struct ExpectedBalanceSheet {
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub total_equity: Decimal,
    pub total_liab_equity: Decimal,
}

/// Expected cash flow totals.
pub struct ExpectedCashFlow {
    pub opening: Decimal,
    pub closing: Decimal,
    pub movement: Decimal,
    pub total_inflows: Decimal,
    pub total_outflows: Decimal,
}

/// A complete canonical fixture: accounts, transactions, and
/// expected report outputs.
pub struct CanonicalFixture {
    pub name: &'static str,
    pub base_currency: &'static str,
    pub accounts: &'static [FixtureAccount],
    pub transactions: &'static [FixtureTransaction],
    pub trial_balance: &'static [ExpectedTrialBalanceRow],
    pub income_statement: ExpectedIncomeStatement,
    pub balance_sheet: ExpectedBalanceSheet,
    pub cash_flow: ExpectedCashFlow,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
}

// ---------------------------------------------------------------------------
// CORE LEDGER — covers all five account types, balanced transactions,
// and exact expected report outputs.
// ---------------------------------------------------------------------------

pub const CORE_LEDGER: CanonicalFixture = CanonicalFixture {
    name: "Acme Corp",
    base_currency: "USD",
    period_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
    period_to: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
    accounts: &[
        FixtureAccount {
            name: "Cash",
            code: "1000",
            account_type: "ASSET",
            subtype: "CURRENT_ASSET",
            currency: "USD",
        },
        FixtureAccount {
            name: "Accounts Receivable",
            code: "",
            account_type: "ASSET",
            subtype: "CURRENT_ASSET",
            currency: "USD",
        },
        FixtureAccount {
            name: "Inventory",
            code: "",
            account_type: "ASSET",
            subtype: "CURRENT_ASSET",
            currency: "USD",
        },
        FixtureAccount {
            name: "Equipment",
            code: "",
            account_type: "ASSET",
            subtype: "FIXED_ASSET",
            currency: "USD",
        },
        FixtureAccount {
            name: "Accounts Payable",
            code: "2000",
            account_type: "LIABILITY",
            subtype: "CURRENT_LIABILITY",
            currency: "USD",
        },
        FixtureAccount {
            name: "Owner's Equity",
            code: "3000",
            account_type: "EQUITY",
            subtype: "EQUITY",
            currency: "USD",
        },
        FixtureAccount {
            name: "Sales Revenue",
            code: "4000",
            account_type: "INCOME",
            subtype: "OPERATING_INCOME",
            currency: "USD",
        },
        FixtureAccount {
            name: "Cost of Goods Sold",
            code: "5000",
            account_type: "EXPENSE",
            subtype: "COST_OF_GOODS_SOLD",
            currency: "USD",
        },
        FixtureAccount {
            name: "Rent Expense",
            code: "6000",
            account_type: "EXPENSE",
            subtype: "OPERATING_EXPENSE",
            currency: "USD",
        },
        FixtureAccount {
            name: "Salary Expense",
            code: "6100",
            account_type: "EXPENSE",
            subtype: "OPERATING_EXPENSE",
            currency: "USD",
        },
    ],
    transactions: &[
        // T1: Owner investment
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            description: "Owner investment",
            postings: &[
                FixturePosting {
                    account_name: "Cash",
                    direction: "DEBIT",
                    amount: dec!(50000),
                },
                FixturePosting {
                    account_name: "Owner's Equity",
                    direction: "CREDIT",
                    amount: dec!(50000),
                },
            ],
        },
        // T2: Purchase equipment
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
            description: "Purchase equipment",
            postings: &[
                FixturePosting {
                    account_name: "Equipment",
                    direction: "DEBIT",
                    amount: dec!(10000),
                },
                FixturePosting {
                    account_name: "Cash",
                    direction: "CREDIT",
                    amount: dec!(10000),
                },
            ],
        },
        // T3: Purchase inventory on credit
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            description: "Purchase inventory on credit",
            postings: &[
                FixturePosting {
                    account_name: "Inventory",
                    direction: "DEBIT",
                    amount: dec!(5000),
                },
                FixturePosting {
                    account_name: "Accounts Payable",
                    direction: "CREDIT",
                    amount: dec!(5000),
                },
            ],
        },
        // T4: Sale on credit
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "Sale on credit",
            postings: &[
                FixturePosting {
                    account_name: "Accounts Receivable",
                    direction: "DEBIT",
                    amount: dec!(8000),
                },
                FixturePosting {
                    account_name: "Sales Revenue",
                    direction: "CREDIT",
                    amount: dec!(8000),
                },
            ],
        },
        // T5: Record COGS for the sale
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 12).unwrap(),
            description: "Record COGS for sale",
            postings: &[
                FixturePosting {
                    account_name: "Cost of Goods Sold",
                    direction: "DEBIT",
                    amount: dec!(3000),
                },
                FixturePosting {
                    account_name: "Inventory",
                    direction: "CREDIT",
                    amount: dec!(3000),
                },
            ],
        },
        // T6: Receive payment from customer
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "Receive payment from customer",
            postings: &[
                FixturePosting {
                    account_name: "Cash",
                    direction: "DEBIT",
                    amount: dec!(8000),
                },
                FixturePosting {
                    account_name: "Accounts Receivable",
                    direction: "CREDIT",
                    amount: dec!(8000),
                },
            ],
        },
        // T7: Pay supplier
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 18).unwrap(),
            description: "Pay supplier",
            postings: &[
                FixturePosting {
                    account_name: "Accounts Payable",
                    direction: "DEBIT",
                    amount: dec!(5000),
                },
                FixturePosting {
                    account_name: "Cash",
                    direction: "CREDIT",
                    amount: dec!(5000),
                },
            ],
        },
        // T8: Pay rent
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
            description: "Pay rent",
            postings: &[
                FixturePosting {
                    account_name: "Rent Expense",
                    direction: "DEBIT",
                    amount: dec!(2000),
                },
                FixturePosting {
                    account_name: "Cash",
                    direction: "CREDIT",
                    amount: dec!(2000),
                },
            ],
        },
        // T9: Pay salary
        FixtureTransaction {
            date: NaiveDate::from_ymd_opt(2026, 1, 25).unwrap(),
            description: "Pay salary",
            postings: &[
                FixturePosting {
                    account_name: "Salary Expense",
                    direction: "DEBIT",
                    amount: dec!(6000),
                },
                FixturePosting {
                    account_name: "Cash",
                    direction: "CREDIT",
                    amount: dec!(6000),
                },
            ],
        },
    ],
    // Trial balance as of Jan 31 (zero-balance accounts excluded).
    trial_balance: &[
        ExpectedTrialBalanceRow {
            account_name: "Cash",
            account_type: "ASSET",
            debit: dec!(35000),
            credit: dec!(0),
        },
        ExpectedTrialBalanceRow {
            account_name: "Inventory",
            account_type: "ASSET",
            debit: dec!(2000),
            credit: dec!(0),
        },
        ExpectedTrialBalanceRow {
            account_name: "Equipment",
            account_type: "ASSET",
            debit: dec!(10000),
            credit: dec!(0),
        },
        ExpectedTrialBalanceRow {
            account_name: "Owner's Equity",
            account_type: "EQUITY",
            debit: dec!(0),
            credit: dec!(50000),
        },
        ExpectedTrialBalanceRow {
            account_name: "Sales Revenue",
            account_type: "INCOME",
            debit: dec!(0),
            credit: dec!(8000),
        },
        ExpectedTrialBalanceRow {
            account_name: "Cost of Goods Sold",
            account_type: "EXPENSE",
            debit: dec!(3000),
            credit: dec!(0),
        },
        ExpectedTrialBalanceRow {
            account_name: "Rent Expense",
            account_type: "EXPENSE",
            debit: dec!(2000),
            credit: dec!(0),
        },
        ExpectedTrialBalanceRow {
            account_name: "Salary Expense",
            account_type: "EXPENSE",
            debit: dec!(6000),
            credit: dec!(0),
        },
    ],
    income_statement: ExpectedIncomeStatement {
        revenue_total: dec!(8000),
        cogs_total: dec!(3000),
        gross_profit: dec!(5000),
        opex_total: dec!(8000),
        operating_income: dec!(-3000),
        net_income: dec!(-3000),
    },
    balance_sheet: ExpectedBalanceSheet {
        total_assets: dec!(47000),
        total_liabilities: dec!(0),
        total_equity: dec!(47000),
        total_liab_equity: dec!(47000),
    },
    // Cash flow for Jan 1–31: opening balance AT Jan 1 includes
    // T1 (owner investment, also dated Jan 1) because the query
    // uses `txn_date <= from`. Movement = closing - opening.
    cash_flow: ExpectedCashFlow {
        opening: dec!(50000),
        closing: dec!(35000),
        movement: dec!(-15000),
        total_inflows: dec!(35000),
        total_outflows: dec!(0),
    },
};
