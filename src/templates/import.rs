use askama::Template;
use uuid::Uuid;

use crate::domain::Account;

use crate::handlers::import::{CsvMapping, ParsedRow};
use crate::handlers::import_wizard::{ColumnMap, TransformedRow};

#[derive(Template)]
#[template(path = "import/upload.html")]
pub struct ImportUpload {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "import/preview.html")]
pub struct ImportPreview {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub filename: String,
    /// "csv", "ofx", "qif", or "mt940". Shown in the preview
    /// header so the user knows which parser produced the
    /// rows.
    pub format: String,
    pub headers: Vec<String>,
    pub rows: Vec<ParsedRow>,
    pub accounts: Vec<Account>,
    pub mapping: CsvMapping,
    pub error: String,
}

#[derive(Template)]
#[template(path = "import/wizard_map.html")]
pub struct WizardMapPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub headers: Vec<String>,
    pub map: ColumnMap,
    pub csv_content: String,
    pub rows_preview: Vec<Vec<String>>,
    pub saved_mapping_name: String,
}

#[derive(Template)]
#[template(path = "import/wizard_preview.html")]
pub struct WizardPreviewPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub headers: Vec<String>,
    pub map: ColumnMap,
    pub csv_content: String,
    pub rows: Vec<TransformedRow>,
}
