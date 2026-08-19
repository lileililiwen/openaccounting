use askama::Template;
use uuid::Uuid;

use crate::domain::{Contact, Invoice};

#[derive(Template)]
#[template(path = "invoices/list.html")]
pub struct InvoiceList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub invoices: Vec<(Invoice, String)>,
    pub kind_filter: String,
}

#[derive(Template)]
#[template(path = "invoices/new.html")]
pub struct InvoiceNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub contacts: Vec<Contact>,
    pub error: String,
}
