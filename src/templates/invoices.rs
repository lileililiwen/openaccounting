use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
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
    /// Today, for the computed overdue flag (`a18-invoicing-upgrade`).
    pub today: NaiveDate,
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

#[derive(Template)]
#[template(path = "invoices/show.html")]
pub struct InvoiceShow {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub invoice: Invoice,
    pub contact_name: String,
    /// `(description, quantity, unit_price, amount)`.
    pub lines: Vec<(String, Decimal, Decimal, Decimal)>,
    pub outstanding: Decimal,
    pub overdue: bool,
}
