use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "UPPERCASE")]
pub enum Direction {
    Debit,
    Credit,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Debit => "DEBIT",
            Direction::Credit => "CREDIT",
        }
    }
    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "DEBIT" => Direction::Debit,
            "CREDIT" => Direction::Credit,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Posting {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub amount: Decimal,
    pub direction: String,
    pub memo: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Posting {
    pub fn direction(&self) -> Option<Direction> {
        Direction::from_db(&self.direction)
    }
}
