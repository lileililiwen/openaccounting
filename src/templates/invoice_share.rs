use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct SharedDocRow {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}

/// Login-free public view of a shared invoice/estimate.
#[derive(Template)]
#[template(path = "invoice_share/public.html")]
pub struct PublicDocumentPage {
    pub doc_kind: String,
    pub number: String,
    pub due_date: NaiveDate,
    pub total: Decimal,
    pub status: String,
    pub lines: Vec<SharedDocRow>,
    pub is_estimate: bool,
    /// "accepted" / "declined" after a decision POST.
    pub decision: String,
}
