use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::Account;

#[derive(Template)]
#[template(path = "transactions/list.html")]
pub struct TransactionList {
    pub user_id: Uuid,
    pub username: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub transactions: Vec<TransactionRow>,
    pub filter: TransactionFilter,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TransactionRow {
    pub id: Uuid,
    pub date: NaiveDate,
    pub description: String,
    pub payee: String,
    pub currency: String,
    pub total: Decimal,
    pub doc_count: i64,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TransactionFilter {
    pub from: String,
    pub to: String,
    pub account_id: String,
    pub q: String,
}

#[derive(Template)]
#[template(path = "transactions/new.html")]
pub struct TransactionNew {
    pub user_id: Uuid,
    pub username: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub currency: String,
    pub accounts: Vec<Account>,
    pub error: String,
    pub form: TransactionForm,
}

#[derive(Clone, Debug, Default)]
pub struct TransactionForm {
    pub date: String,
    pub description: String,
    pub payee: String,
    pub reference: String,
    pub lines: Vec<TransactionFormLine>,
}

impl TransactionForm {
    pub fn empty() -> Self {
        Self {
            date: chrono::Utc::now().date_naive().to_string(),
            description: String::new(),
            payee: String::new(),
            reference: String::new(),
            lines: vec![TransactionFormLine::default(); 2],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TransactionFormLine {
    pub account_id: String,
    pub amount: String,
    pub direction: String,
    pub memo: String,
}

#[derive(Template)]
#[template(path = "transactions/show.html")]
pub struct TransactionShow {
    pub user_id: Uuid,
    pub username: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub txn_id: Uuid,
    pub date: NaiveDate,
    pub description: String,
    pub payee: String,
    pub reference: String,
    pub currency: String,
    pub lines: Vec<TransactionShowLine>,
    pub documents: Vec<crate::domain::Document>,
    pub tags: Vec<String>,
    pub flash: String,
}

#[derive(Clone, Debug)]
pub struct TransactionShowLine {
    pub account_name: String,
    pub account_type: String,
    pub direction: String,
    pub amount: Decimal,
    pub memo: String,
}
