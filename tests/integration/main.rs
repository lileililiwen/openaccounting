// Integration tests are test code. The production lints forbid
// `unwrap` / `expect` / `panic`; the test code is allowed to use
// them, and the fixture itself relies on them for clean failure
// modes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../common/mod.rs"]
mod common;

mod account_management;
mod admin;
mod amortization;
mod api;
mod append_only;
mod approval_routing;
mod audit_chain;
mod bank_feeds;
mod bank_statement_imports;
mod cash_basis;
mod cash_basis_recognize;
mod cash_flow_forecast;
mod csrf;
mod csv_import_completion;
mod csv_import_wizard;
mod dark_mode;
mod dashboard_layout;
mod document_authorization;
mod document_ocr;
mod empty_states;
mod expense_reimbursement;
mod export;
mod health;
mod import_alipay;
mod import_wechat;
mod investment_lots;
mod invoices_upgrade;
mod keyboard_shortcuts;
mod localization;
mod login_throttle;
mod menu_navigation;
mod metrics;
mod notification_preferences;
mod notifications;
mod onboarding;
mod ocr_feedback;
mod password_strength;
mod policies;
mod posting_service;
mod printable_views;
mod pta_round_trip;
mod pwa;
mod reconciliation_rules;
mod reports_polish;
mod role_enforcement;
mod s3_store;
mod saved_searches;
mod scheduled_backup;
mod secure_cookie;
mod session_timeout;
mod signed_cookies;
mod smoke;
mod totp;
mod transaction_number;
mod transactions_bulk;
mod transactions_document;
mod transactions_draft;
mod transactions_edit;
mod transactions_split;
mod transfers;
mod tax_transactions;
mod stub_cleanup;
mod upload_validation;
