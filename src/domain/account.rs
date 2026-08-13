use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "UPPERCASE")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Asset => "ASSET",
            AccountType::Liability => "LIABILITY",
            AccountType::Equity => "EQUITY",
            AccountType::Income => "INCOME",
            AccountType::Expense => "EXPENSE",
        }
    }
    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "ASSET" => AccountType::Asset,
            "LIABILITY" => AccountType::Liability,
            "EQUITY" => AccountType::Equity,
            "INCOME" => AccountType::Income,
            "EXPENSE" => AccountType::Expense,
            _ => return None,
        })
    }
    /// Normal balance side. Asset/Expense are debit-normal; the rest are credit-normal.
    pub fn normal_direction(&self) -> crate::domain::posting::Direction {
        use crate::domain::posting::Direction;
        match self {
            AccountType::Asset | AccountType::Expense => Direction::Debit,
            AccountType::Liability | AccountType::Equity | AccountType::Income => Direction::Credit,
        }
    }
    pub fn display_label(&self) -> &'static str {
        match self {
            AccountType::Asset => "Asset",
            AccountType::Liability => "Liability",
            AccountType::Equity => "Equity",
            AccountType::Income => "Income",
            AccountType::Expense => "Expense",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub code: Option<String>,
    pub r#type: String,
    pub currency: String,
    pub is_archived: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Account {
    pub fn account_type(&self) -> Option<AccountType> {
        AccountType::from_db(&self.r#type)
    }
}
