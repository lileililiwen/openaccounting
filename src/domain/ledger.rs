use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::account::{AccountSubtype, AccountType};

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ledger {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Ledger {
    pub fn default_chart_of_accounts(currency: &str) -> Vec<NewAccount> {
        use AccountSubtype::{CurrentAsset, CurrentLiability, OperatingExpense, OperatingIncome};
        use AccountType::{Asset, Expense, Income, Liability};
        vec![
            ("1000", "Cash on Hand", Asset, CurrentAsset, false),
            ("1010", "Bank Account", Asset, CurrentAsset, false),
            ("1200", "Accounts Receivable", Asset, CurrentAsset, false),
            ("2000", "Accounts Payable", Liability, CurrentLiability, false),
            ("2100", "Credit Card", Liability, CurrentLiability, false),
            ("3000", "Owner's Equity", AccountType::Equity, AccountSubtype::Equity, false),
            ("3010", "Opening Balances", AccountType::Equity, AccountSubtype::Equity, false),
            ("4000", "Sales Revenue", Income, OperatingIncome, false),
            ("4900", "Other Income", Income, OperatingIncome, false),
            ("5000", "Office Supplies", Expense, OperatingExpense, false),
            ("5100", "Travel & Meals", Expense, OperatingExpense, false),
            ("5200", "Software & SaaS", Expense, OperatingExpense, false),
            ("5300", "Marketing", Expense, OperatingExpense, false),
            ("5400", "Professional Services", Expense, OperatingExpense, false),
            ("5500", "Rent", Expense, OperatingExpense, false),
            ("5600", "Utilities", Expense, OperatingExpense, false),
            ("5900", "Other Expense", Expense, OperatingExpense, false),
        ]
        .into_iter()
        .map(|(code, name, ty, subty, archived)| NewAccount {
            name: name.to_string(),
            code: Some(code.to_string()),
            account_type: ty,
            account_subtype: subty,
            currency: currency.to_string(),
            is_archived: archived,
            description: None,
            parent_id: None,
        })
        .collect()
    }
}

#[derive(Clone, Debug)]
pub struct NewAccount {
    pub name: String,
    pub code: Option<String>,
    pub account_type: AccountType,
    pub account_subtype: AccountSubtype,
    pub currency: String,
    pub is_archived: bool,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
}
