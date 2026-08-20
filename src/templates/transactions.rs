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
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub transactions: Vec<TransactionRow>,
    pub filter: TransactionFilter,
    /// The original raw query string (e.g.
    /// `from=2026-01-01&q=coffee`). Empty when the user landed
    /// on the bare URL. Used by the saved-searches partial to
    /// know what to persist.
    pub current_query: String,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TransactionRow {
    pub id: Uuid,
    pub date: NaiveDate,
    pub description: String,
    pub payee: String,
    pub currency: String,
    pub kind: String,
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
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub currency: String,
    /// Accounts grouped by type for an `<optgroup>`-labelled picker
    /// (`ux-transaction-entry`). Each entry is `(type label, accounts)`.
    pub account_groups: Vec<(String, Vec<Account>)>,
    pub error: String,
    /// When the bind page linked here with `?bind_doc=<id>`, the
    /// created transaction is bound to that unbound document
    /// (`a13-document-inbox`).
    pub bind_doc: String,
    pub form: TransactionForm,
}

#[derive(Clone, Debug, Default)]
pub struct TransactionForm {
    pub date: String,
    pub description: String,
    pub payee: String,
    pub reference: String,
    /// Optional human-citable number (`a5-transaction-numbering`).
    pub number: String,
    pub lines: Vec<TransactionFormLine>,
}

impl TransactionForm {
    pub fn empty() -> Self {
        Self {
            date: chrono::Utc::now().date_naive().to_string(),
            description: String::new(),
            payee: String::new(),
            reference: String::new(),
            number: String::new(),
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
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
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
    pub template_id: Option<Uuid>,
    pub template_description: String,
}

#[derive(Clone, Debug)]
pub struct TransactionShowLine {
    pub account_name: String,
    pub account_type: String,
    pub direction: String,
    pub amount: Decimal,
    pub memo: String,
}
