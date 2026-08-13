use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::account::AccountType;

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
        use AccountType::*;
        vec![
            ("1000", "Cash on Hand", Asset, false),
            ("1010", "Bank Account", Asset, false),
            ("1200", "Accounts Receivable", Asset, false),
            ("2000", "Accounts Payable", Liability, false),
            ("2100", "Credit Card", Liability, false),
            ("3000", "Owner's Equity", Equity, false),
            ("3010", "Opening Balances", Equity, false),
            ("4000", "Sales Revenue", Income, false),
            ("4900", "Other Income", Income, false),
            ("5000", "Office Supplies", Expense, false),
            ("5100", "Travel & Meals", Expense, false),
            ("5200", "Software & SaaS", Expense, false),
            ("5300", "Marketing", Expense, false),
            ("5400", "Professional Services", Expense, false),
            ("5500", "Rent", Expense, false),
            ("5600", "Utilities", Expense, false),
            ("5900", "Other Expense", Expense, false),
        ]
        .into_iter()
        .map(|(code, name, ty, archived)| NewAccount {
            name: name.to_string(),
            code: Some(code.to_string()),
            account_type: ty,
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
    pub currency: String,
    pub is_archived: bool,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
}
