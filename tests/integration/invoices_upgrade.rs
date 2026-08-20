//! HTTP integration tests for the invoicing upgrade (`a18-invoicing-upgrade`):
//! line items, detail page, overdue flag, mark-paid/void, payment shortcut.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "X-OA-CSRF-Bypass",
                reqwest::header::HeaderValue::from_static("1"),
            );
            h
        })
        .build()
        .unwrap()
}

async fn setup(server: &TestServer, tag: &str) -> (reqwest::Client, Uuid, Uuid) {
    let client = make_client();
    let email = format!("{tag}@example.com");
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("username", tag),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .expect("register");
    client
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .expect("login");

    let resp = client
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", format!("{tag}-books")),
            ("base_currency", "USD".to_string()),
            ("timezone", "UTC".to_string()),
            ("basis", "accrual".to_string()),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let ledger_id: Uuid = loc.rsplit('/').next().unwrap().parse().unwrap();

    // Create a contact (invoices require one).
    let resp = client
        .post(format!("{}/ledgers/{ledger_id}/contacts/new", server.base_url()))
        .form(&[
            ("name", "Acme Corp"),
            ("kind", "customer"),
            ("email", "acme@example.com"),
            ("phone", ""),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let pool = server.db().pool();
    let (contact_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM contacts WHERE ledger_id = $1 AND name = 'Acme Corp'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    (client, ledger_id, contact_id)
}

async fn create_invoice(
    client: &reqwest::Client,
    base: &str,
    ledger_id: Uuid,
    contact_id: Uuid,
    due_date: &str,
) -> reqwest::Response {
    let cid = contact_id.to_string();
    client
        .post(format!("{base}/ledgers/{ledger_id}/invoices/new"))
        .form(&[
            ("contact_id", cid.as_str()),
            ("kind", "receivable"),
            ("invoice_number", "INV-1001"),
            ("invoice_date", "2026-08-01"),
            ("due_date", due_date),
            ("lines[0][description]", "Consulting"),
            ("lines[0][quantity]", "10"),
            ("lines[0][unit_price]", "150.00"),
            ("lines[1][description]", "Reimbursable materials"),
            ("lines[1][quantity]", "1"),
            ("lines[1][unit_price]", "100.00"),
        ])
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn create_invoice_with_lines_stores_total_and_lines() {
    let server = TestServer::new().await;
    let (client, ledger_id, contact_id) = setup(&server, "invc").await;
    let base = server.base_url();
    let pool = server.db().pool();

    let resp = create_invoice(&client, &base, ledger_id, contact_id, "2026-09-01").await;
    assert_eq!(resp.status(), 303);

    let (total,): (Decimal,) = sqlx::query_as(
        "SELECT total FROM invoices WHERE ledger_id = $1 AND invoice_number = 'INV-1001'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, Decimal::new(1600, 0), "10×150 + 1×100");

    let (line_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invoice_lines il
         JOIN invoices i ON i.id = il.invoice_id WHERE i.ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(line_count, 2);
}

#[tokio::test]
async fn create_invoice_without_lines_rejected() {
    let server = TestServer::new().await;
    let (client, ledger_id, contact_id) = setup(&server, "invnolines").await;
    let base = server.base_url();

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/invoices/new"))
        .form(&[
            ("contact_id", contact_id.to_string().as_str()),
            ("kind", "receivable"),
            ("invoice_date", "2026-08-01"),
            ("due_date", "2026-09-01"),
        ])
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Add at least one line item"),
        "expected line-item error, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn detail_page_shows_lines_totals_and_paid() {
    let server = TestServer::new().await;
    let (client, ledger_id, contact_id) = setup(&server, "invdet").await;
    let base = server.base_url();
    let pool = server.db().pool();
    create_invoice(&client, &base, ledger_id, contact_id, "2026-09-01").await;

    let invoice_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM invoices WHERE ledger_id = $1 AND invoice_number = 'INV-1001'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let body = client
        .get(format!("{base}/ledgers/{ledger_id}/invoices/{invoice_id}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("Consulting"), "line item missing");
    assert!(body.contains("1600.0000"), "total missing");
    assert!(body.contains("Outstanding"), "outstanding row missing");
    assert!(
        body.contains("Record payment") && body.contains("invoice_id="),
        "payment shortcut missing"
    );
}

#[tokio::test]
async fn overdue_flag_appears_and_hides() {
    let server = TestServer::new().await;
    let (client, ledger_id, contact_id) = setup(&server, "invod").await;
    let base = server.base_url();
    let pool = server.db().pool();

    // Past due invoice.
    create_invoice(&client, &base, ledger_id, contact_id, "2020-01-01").await;
    // Future-due invoice.
    let cid = contact_id.to_string();
    client
        .post(format!("{base}/ledgers/{ledger_id}/invoices/new"))
        .form(&[
            ("contact_id", cid.as_str()),
            ("kind", "receivable"),
            ("invoice_number", "INV-2001"),
            ("invoice_date", "2026-08-01"),
            ("due_date", "2027-01-01"),
            ("lines[0][description]", "Future work"),
            ("lines[0][quantity]", "1"),
            ("lines[0][unit_price]", "50.00"),
        ])
        .send()
        .await
        .unwrap();

    let list = client
        .get(format!("{base}/ledgers/{ledger_id}/invoices"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(list.contains("overdue"), "past-due invoice should be flagged");

    // The future invoice detail should NOT be overdue.
    let future_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM invoices WHERE ledger_id = $1 AND invoice_number = 'INV-2001'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let detail = client
        .get(format!("{base}/ledgers/{ledger_id}/invoices/{future_id}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!detail.contains("overdue"), "future invoice must not be overdue");
}

#[tokio::test]
async fn mark_paid_and_void_update_status_and_audit() {
    let server = TestServer::new().await;
    let (client, ledger_id, contact_id) = setup(&server, "invpv").await;
    let base = server.base_url();
    let pool = server.db().pool();
    create_invoice(&client, &base, ledger_id, contact_id, "2026-09-01").await;
    let invoice_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM invoices WHERE ledger_id = $1 AND invoice_number = 'INV-1001'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    client
        .post(format!(
            "{base}/ledgers/{ledger_id}/invoices/{invoice_id}/mark-paid"
        ))
        .send()
        .await
        .unwrap();
    let (status, paid): (String, Decimal) =
        sqlx::query_as("SELECT status, amount_paid FROM invoices WHERE id = $1")
            .bind(invoice_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "paid");
    assert_eq!(paid, Decimal::new(1600, 0));

    client
        .post(format!("{base}/ledgers/{ledger_id}/invoices/{invoice_id}/void"))
        .send()
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM invoices WHERE id = $1")
        .bind(invoice_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "void");

    let audit_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_entries WHERE entity_type = 'invoice' AND entity_id = $1",
    )
    .bind(invoice_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_count.0 >= 3, "create + mark_paid + void should be logged");
}

#[tokio::test]
async fn payments_new_page_prefills_invoice() {
    let server = TestServer::new().await;
    let (client, ledger_id, contact_id) = setup(&server, "invpay").await;
    let base = server.base_url();
    let pool = server.db().pool();
    create_invoice(&client, &base, ledger_id, contact_id, "2026-09-01").await;
    let invoice_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM invoices WHERE ledger_id = $1 AND invoice_number = 'INV-1001'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // The page must load (the `number` → `invoice_number` fix) and
    // preselect the invoice.
    let body = client
        .get(format!(
            "{base}/ledgers/{ledger_id}/payments/new?invoice_id={invoice_id}"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("INV-1001"),
        "invoice option missing, got: {}",
        body.chars().take(200).collect::<String>()
    );
}
