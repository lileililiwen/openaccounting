use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

/// Askama context for the OCR result page (`templates/documents/ocr.html`).
#[derive(Template)]
#[template(path = "documents/ocr.html")]
pub struct DocumentOcrPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub document_id: Uuid,
    pub filename: String,
    /// `None` means the OCR job is still pending.
    pub result: Option<OcrResultView>,
}

#[derive(Debug, Clone)]
pub struct OcrResultView {
    pub amount: Option<Decimal>,
    pub txn_date: Option<NaiveDate>,
    pub merchant: Option<String>,
    pub raw_text: String,
    pub confidence: f32,
    pub error_message: Option<String>,
    /// Reimbursement claim IDs available to apply this result to.
    pub claim_id: Option<Uuid>,
}
