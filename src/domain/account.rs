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
    /// Returns the valid subtypes for this account type.
    pub fn valid_subtypes(&self) -> &'static [AccountSubtype] {
        match self {
            AccountType::Asset => &[
                AccountSubtype::CurrentAsset,
                AccountSubtype::FixedAsset,
                AccountSubtype::IntangibleAsset,
                AccountSubtype::OtherAsset,
            ],
            AccountType::Liability => &[
                AccountSubtype::CurrentLiability,
                AccountSubtype::LongTermLiability,
            ],
            AccountType::Equity => &[
                AccountSubtype::Equity,
                AccountSubtype::RetainedEarnings,
                AccountSubtype::Drawing,
            ],
            AccountType::Income => &[
                AccountSubtype::OperatingIncome,
                AccountSubtype::NonOperatingIncome,
            ],
            AccountType::Expense => &[
                AccountSubtype::CostOfGoodsSold,
                AccountSubtype::OperatingExpense,
                AccountSubtype::NonOperatingExpense,
                AccountSubtype::TaxExpense,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountSubtype {
    // Asset subtypes
    CurrentAsset,
    FixedAsset,
    IntangibleAsset,
    OtherAsset,
    // Liability subtypes
    CurrentLiability,
    LongTermLiability,
    // Equity subtypes
    Equity,
    RetainedEarnings,
    Drawing,
    // Income subtypes
    OperatingIncome,
    NonOperatingIncome,
    // Expense subtypes
    CostOfGoodsSold,
    OperatingExpense,
    NonOperatingExpense,
    TaxExpense,
}

impl AccountSubtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CurrentAsset => "CURRENT_ASSET",
            Self::FixedAsset => "FIXED_ASSET",
            Self::IntangibleAsset => "INTANGIBLE_ASSET",
            Self::OtherAsset => "OTHER_ASSET",
            Self::CurrentLiability => "CURRENT_LIABILITY",
            Self::LongTermLiability => "LONG_TERM_LIABILITY",
            Self::Equity => "EQUITY",
            Self::RetainedEarnings => "RETAINED_EARNINGS",
            Self::Drawing => "DRAWING",
            Self::OperatingIncome => "OPERATING_INCOME",
            Self::NonOperatingIncome => "NON_OPERATING_INCOME",
            Self::CostOfGoodsSold => "COST_OF_GOODS_SOLD",
            Self::OperatingExpense => "OPERATING_EXPENSE",
            Self::NonOperatingExpense => "NON_OPERATING_EXPENSE",
            Self::TaxExpense => "TAX_EXPENSE",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "CURRENT_ASSET" => Self::CurrentAsset,
            "FIXED_ASSET" => Self::FixedAsset,
            "INTANGIBLE_ASSET" => Self::IntangibleAsset,
            "OTHER_ASSET" => Self::OtherAsset,
            "CURRENT_LIABILITY" => Self::CurrentLiability,
            "LONG_TERM_LIABILITY" => Self::LongTermLiability,
            "EQUITY" => Self::Equity,
            "RETAINED_EARNINGS" => Self::RetainedEarnings,
            "DRAWING" => Self::Drawing,
            "OPERATING_INCOME" => Self::OperatingIncome,
            "NON_OPERATING_INCOME" => Self::NonOperatingIncome,
            "COST_OF_GOODS_SOLD" => Self::CostOfGoodsSold,
            "OPERATING_EXPENSE" => Self::OperatingExpense,
            "NON_OPERATING_EXPENSE" => Self::NonOperatingExpense,
            "TAX_EXPENSE" => Self::TaxExpense,
            _ => return None,
        })
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::CurrentAsset => "Current Asset",
            Self::FixedAsset => "Fixed Asset",
            Self::IntangibleAsset => "Intangible Asset",
            Self::OtherAsset => "Other Asset",
            Self::CurrentLiability => "Current Liability",
            Self::LongTermLiability => "Long-Term Liability",
            Self::Equity => "Equity",
            Self::RetainedEarnings => "Retained Earnings",
            Self::Drawing => "Drawing",
            Self::OperatingIncome => "Operating Income",
            Self::NonOperatingIncome => "Non-Operating Income",
            Self::CostOfGoodsSold => "Cost of Goods Sold",
            Self::OperatingExpense => "Operating Expense",
            Self::NonOperatingExpense => "Non-Operating Expense",
            Self::TaxExpense => "Tax Expense",
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
    pub subtype: String,
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

    pub fn account_subtype(&self) -> Option<AccountSubtype> {
        AccountSubtype::from_db(&self.subtype)
    }
}
