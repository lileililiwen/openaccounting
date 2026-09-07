//! Integration tests for `ar-getting-paid`: estimates/convert, share
//! links, recurring invoices, e-invoice exports.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";
const D: fn(i32, u32, u32) -> NaiveDate = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();

async fn bootstrap(server: &TestServer) -> (String, Uuid, Uuid) {
    let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
    let email = format!("ar-{suffix}@example.com");
    let cookie = server.bootstrap_user(&email, &email, PASSWORD).await;
    let pool = server.db().pool();
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", format!("AR Co {suffix}").as_str()),
            ("base_currency", "EUR"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    // Seller identity for e-invoicing (EN 16931 mandates seller VAT).
    sqlx::query("UPDATE ledgers SET vat_id = 'DE123456789', country_code = 'DE' WHERE name = $1")
        .bind(format!("AR Co {suffix}"))
        .execute(&pool)
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();
    let (contact_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO contacts (ledger_id, name, email, kind, vat_id, country_code)
           VALUES ($1, 'Buyer SA', 'buyer@example.com', 'customer', 'FR09876543210', 'FR')
           RETURNING id"#,
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (cookie, ledger_id, contact_id)
}

async fn create_estimate(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    contact_id: Uuid,
) -> Uuid {
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/estimates",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("contact_id", contact_id.to_string().as_str()),
            ("invoice_date", "2026-08-01"),
            ("due_date", "2026-08-31"),
            ("description", "Widget"),
            ("quantity", "2"),
            ("unit_price", "50.00"),
            ("description", "Setup"),
            ("quantity", "1"),
            ("unit_price", "25.50"),
        ])
        .send()
        .await
        .unwrap();
    let st = resp.status();
    let dbg = if st != 303 {
        resp.text()
            .await
            .unwrap_or_default()
            .chars()
            .take(400)
            .collect()
    } else {
        String::new()
    };
    assert_eq!(st, 303, "{dbg}");
    let pool = server.db().pool();
    let (id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM invoices WHERE ledger_id = $1 AND doc_kind = 'estimate'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn estimate_convert_creates_invoice_with_identical_lines() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, contact_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    let estimate_id = create_estimate(&server, &cookie, ledger_id, contact_id).await;

    // Convert.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/estimates/{estimate_id}/convert",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    // Invoice exists with identical lines; estimate is converted + linked.
    let (inv_total,): (Decimal,) =
        sqlx::query_as("SELECT total FROM invoices WHERE ledger_id = $1 AND doc_kind = 'invoice'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(inv_total, Decimal::new(12550, 2));

    let line_counts: (i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM invoice_lines WHERE invoice_id IN
                (SELECT id FROM invoices WHERE ledger_id = $1 AND doc_kind = 'invoice')),
             (SELECT COUNT(*) FROM invoice_lines WHERE invoice_id = $2)"#,
    )
    .bind(ledger_id)
    .bind(estimate_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(line_counts.0, line_counts.1, "lines copied identically");

    let (status,): (String,) = sqlx::query_as("SELECT status FROM invoices WHERE id = $1")
        .bind(estimate_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "converted");

    // Double conversion → 409 and no second invoice.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/estimates/{estimate_id}/convert",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invoices WHERE ledger_id = $1 AND doc_kind = 'invoice'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 1);
}

#[tokio::test]
async fn share_link_public_view_and_revocation() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, contact_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    // A real invoice (not estimate).
    let (invoice_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number, invoice_date,
                                 due_date, total, status, doc_kind)
           VALUES ($1, $2, 'receivable', 'INV-7', '2026-08-01', '2026-08-31', 500.00, 'open', 'invoice')
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
         VALUES ($1, 'Consulting', 10, 50.00, 500.00, 0)",
    )
    .bind(invoice_id)
    .execute(&pool)
    .await
    .unwrap();

    // Mint a link; the redirect carries the plaintext token once.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/invoices/{invoice_id}/share",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let token = loc.rsplit("shared=").next().unwrap().to_string();
    assert_eq!(token.len(), 64);

    // Token stored hashed, not plaintext.
    let (stored,): (String,) =
        sqlx::query_as("SELECT token_hash FROM invoice_shares WHERE invoice_id = $1")
            .bind(invoice_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(stored, token);

    // Public page renders without auth.
    let fresh = reqwest::Client::builder().build().unwrap();
    let resp = fresh
        .get(format!("{}/share/invoice/{token}", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("INV-7"), "page body: {}", {
        let plain: String = body.chars().filter(|c| c.is_ascii()).collect();
        let start = plain
            .find("<main")
            .or_else(|| plain.find("<h1"))
            .unwrap_or(0);
        plain[start..].chars().take(400).collect::<String>()
    });
    assert!(body.contains("noindex"));

    // Revoke → public access becomes 410.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/invoices/{invoice_id}/share/revoke",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let resp = fresh
        .get(format!("{}/share/invoice/{token}", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 410);

    // Unknown token → 404.
    let other = "0".repeat(64);
    let resp = fresh
        .get(format!("{}/share/invoice/{other}", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn recurring_invoices_issue_once_per_due_date() {
    let server = TestServer::new().await;
    let (_cookie, ledger_id, contact_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Template due 2026-06-01 monthly; today (2026-08) makes three
    // missed months.
    let (tid,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO recurring_invoice_templates
               (ledger_id, contact_id, kind, frequency, next_date, is_active)
           VALUES ($1, $2, 'receivable', 'monthly', '2026-06-01', TRUE) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO recurring_invoice_lines (template_id, description, quantity, unit_price, sort_order)
         VALUES ($1, 'Subscription', 1, 99.00, 0)",
    )
    .bind(tid)
    .execute(&pool)
    .await
    .unwrap();

    openaccounting::jobs::recurring_invoices::scan_and_run(&pool)
        .await
        .unwrap();

    let n: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM invoices WHERE recurring_template_id = $1")
            .bind(tid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n.0, 3, "one invoice per missed month (Jun, Jul, Aug)");

    // Re-run: idempotent.
    openaccounting::jobs::recurring_invoices::scan_and_run(&pool)
        .await
        .unwrap();
    let n2: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM invoices WHERE recurring_template_id = $1")
            .bind(tid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n2.0, 3);

    // Lines copied with correct totals.
    let (total,): (Decimal,) =
        sqlx::query_as("SELECT SUM(total) FROM invoices WHERE recurring_template_id = $1")
            .bind(tid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total, Decimal::new(29700, 2));
}

// ── E-invoicing exports ─────────────────────────────────────────────────

async fn seed_full_invoice(server: &TestServer, ledger_id: Uuid, contact_id: Uuid) -> Uuid {
    let pool = server.db().pool();
    let (invoice_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number, invoice_date,
                                 due_date, total, status, doc_kind, payment_means_code)
           VALUES ($1, $2, 'receivable', '2026-000042', '2026-08-01', '2026-08-31',
                   125.50, 'open', 'invoice', '30')
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (i, (d, q, up)) in [("Widget", "2", "50.00"), ("Setup", "1", "25.50")]
        .iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(invoice_id)
        .bind(d)
        .bind(q.parse::<Decimal>().unwrap())
        .bind(up.parse::<Decimal>().unwrap())
        .bind((q.parse::<Decimal>().unwrap() * up.parse::<Decimal>().unwrap()).round_dp(2))
        .bind(i as i32)
        .execute(&pool)
        .await
        .unwrap();
    }
    invoice_id
}

#[tokio::test]
async fn ubl_export_totals_match_invoice() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, contact_id) = bootstrap(&server).await;
    let invoice_id = seed_full_invoice(&server, ledger_id, contact_id).await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/invoices/{invoice_id}/export.xml?format=ubl",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let st = resp.status();
    let xml = resp.text().await.unwrap_or_default();
    assert_eq!(st, 200, "body={}", {
        let plain: String = xml.chars().filter(|c| c.is_ascii()).collect();
        let start = plain.find("<main").unwrap_or(0);
        plain[start..].chars().take(400).collect::<String>()
    });
    assert!(
        xml.contains("<cbc:PayableAmount currencyID=\"EUR\">125.5</cbc:PayableAmount>"),
        "{xml}"
    );
    assert!(xml.contains("FR09876543210"), "buyer VAT carried through");
    assert!(xml.contains("<cbc:ID>2026-000042</cbc:ID>"));
}

#[tokio::test]
async fn facturx_export_blocks_on_missing_seller_fields() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, contact_id) = bootstrap(&server).await;
    let invoice_id = seed_full_invoice(&server, ledger_id, contact_id).await;

    // Strip the seller identity → CII must refuse with a field-level
    // error list naming exactly what is missing.
    sqlx::query("UPDATE ledgers SET vat_id = NULL WHERE id = $1")
        .bind(ledger_id)
        .execute(&server.db().pool())
        .await
        .unwrap();
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/invoices/{invoice_id}/export.xml?format=facturx",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("seller.vat_id"), "{body}");
}
