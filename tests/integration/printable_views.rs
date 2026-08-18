//! HTTP integration tests for printable views
//! (`u10-printable-views`).
//!
//! Verifies that the print-only CSS hooks are in place: the
//! app stylesheet contains an `@media print` block, the print
//! button partial is wired into at least one report, and the
//! print-only header renders the ledger name + date.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

#[tokio::test]
async fn http_print_css_present() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/static/css/app.css", server.base_url()))
        .send()
        .await
        .expect("GET app.css");
    assert_eq!(resp.status(), 200, "app.css must be served");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("@media print"),
        "app.css must contain an @media print block; got:\n{body}"
    );
    // The hidden-chrome rule must hide .no-print and the
    // logout / theme / notifications forms.
    assert!(
        body.contains(".no-print"),
        "app.css must reference .no-print"
    );
    assert!(
        body.contains("Georgia") || body.contains("serif"),
        "app.css must switch the body to a serif font in print"
    );
    assert!(
        body.contains("page-break-inside: avoid") || body.contains("break-inside: avoid"),
        "app.css must avoid breaking table rows"
    );
    assert!(
        body.contains("print-page-break-before"),
        "app.css must define the totals page-break helper"
    );
}

#[tokio::test]
async fn http_balance_sheet_has_print_button_and_header() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_pv",
            "alice_pv@example.com",
            "correct horse battery staple",
        )
        .await;

    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Print Test"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location")
        .to_string();
    let ledger_id = uuid::Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/balance-sheet",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("GET balance sheet");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();

    // Print button partial present.
    assert!(
        body.contains("window.print()"),
        "balance-sheet page must expose the print button"
    );
    // Print-only header strip with the ledger name + the
    // current date.
    assert!(
        body.contains("Print Test") && body.contains("Printed"),
        "balance-sheet must include a print-only header with ledger name + date"
    );
    assert!(
        body.contains("print-page-break-before"),
        "balance-sheet must mark totals with print-page-break-before"
    );
}
