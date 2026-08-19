use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "payments/list.html")]
pub struct PaymentList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub unapplied: Vec<PaymentRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "payments/form.html")]
pub struct PaymentForm {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub contacts: Vec<(Uuid, String)>,
    pub invoices: Vec<(Uuid, String, Decimal)>,
    pub amount: String,
    pub payment_date: String,
    pub payment_method: String,
    pub reference: String,
    pub kind: String,
    pub contact_id: String,
    pub invoice_id: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "payments/register.html")]
pub struct PaymentRegister {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub payments: Vec<PaymentRow>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct PaymentRow {
    pub id: Uuid,
    pub amount: Decimal,
    pub payment_date: NaiveDate,
    pub payment_method: String,
    pub reference: String,
    pub kind: String,
    pub contact_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub transaction_id: Option<Uuid>,
    pub contact_name: String,
    pub invoice_number: String,
    pub unapplied: bool,
}
