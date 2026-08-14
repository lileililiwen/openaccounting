// Integration tests are test code. The production lints forbid
// `unwrap` / `expect` / `panic`; the test code is allowed to use
// them, and the fixture itself relies on them for clean failure
// modes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../common/mod.rs"]
mod common;

mod approval_routing;
mod bank_feeds;
mod bank_statement_imports;
mod cash_basis;
mod cash_flow_forecast;
mod csv_import_completion;
mod document_ocr;
mod expense_reimbursement;
mod import_alipay;
mod import_wechat;
mod policies;
mod pwa;
mod reconciliation_rules;
mod smoke;
