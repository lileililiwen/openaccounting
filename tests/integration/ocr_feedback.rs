//! HTTP integration tests for the OCR feedback loop
//! (`o7-ocr-feedback`).
//!
//! Verifies that:
//! - A successful apply writes one row to `ocr_corrections`
//!   with both the engine output and the user-edited values.
//! - Setting `OCR_FEEDBACK=false` short-circuits capture.
//! - The corpus export is admin-only and returns a JSON array.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

/// Create a ledger via the public HTTP endpoint so the default
/// chart of accounts (incl. "Other Expense") is seeded. Returns
/// the new ledger id parsed from the Location header.
async fn make_ledger(server: &TestServer, cookie: &str, name: &str) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", name),
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
        .expect("Location header")
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

/// Insert a minimal transaction so the document we upload
/// has something to attach to.
async fn make_txn(server: &TestServer, ledger_id: Uuid, owner_id: Uuid) -> Uuid {
    let pool = server.db().pool();
    sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
         VALUES ($1, '2026-08-18', 'Test txn', 'USD', $2)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap()
}

/// Promote the user with `email` to the Admin role so we can
/// exercise the admin-only corpus export.
async fn promote_admin(server: &TestServer, email: &str) {
    let pool = server.db().pool();
    sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1")
        .bind(email)
        .execute(&pool)
        .await
        .unwrap();
}

/// Insert a transaction, document and synthetic OCR result so
/// the apply endpoint has something to consume.
async fn seed_ocr_apply(
    server: &TestServer,
    owner_email: &str,
    cookie: &str,
) -> (Uuid, Uuid, Uuid) {
    let pool = server.db().pool();
    let (owner_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(owner_email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let ledger_id = make_ledger(server, cookie, "OCR Feedback Co").await;
    let txn_id = make_txn(server, ledger_id, owner_id).await;
    let doc_id: Uuid = sqlx::query_scalar(
        "INSERT INTO documents (transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, category)
         VALUES ($1, 'receipt.png', 'receipt.png', 'image/png', 0, $2, 'Receipt')
         RETURNING id",
    )
    .bind(txn_id)
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO document_ocr_results
               (document_id, amount, txn_date, merchant, raw_text, engine, confidence)
           VALUES ($1, 42.50, '2026-08-18', 'COFFEE SHOP', 'raw', 'tesseract', 85)
           ON CONFLICT (document_id) DO UPDATE SET amount = 42.50, error_message = NULL"#,
    )
    .bind(doc_id)
    .execute(&pool)
    .await
    .unwrap();
    (ledger_id, doc_id, owner_id)
}

/// Helper that creates a draft claim in `ledger_id` and returns
/// its id. Posts directly via the public HTTP endpoint.
async fn create_claim(server: &TestServer, cookie: &str, ledger_id: Uuid) -> Uuid {
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body("title=OCR+Feedback+Test&employee_name=Tester&currency=USD")
        .send()
        .await
        .expect("create claim");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location header")
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

#[tokio::test]
async fn http_ocr_apply_writes_correction() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_fb",
            "alice_fb@example.com",
            "correct horse battery staple",
        )
        .await;
    let (ledger_id, doc_id, _owner_id) =
        seed_ocr_apply(&server, "alice_fb@example.com", &cookie).await;
    let claim_id = create_claim(&server, &cookie, ledger_id).await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/ocr/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[("claim_id", claim_id.to_string())])
        .send()
        .await
        .expect("apply");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status == 303 || status == 302,
        "apply should redirect; got {status} body={body}"
    );

    // Verify exactly one row in `ocr_corrections` for this doc.
    let pool = server.db().pool();
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM ocr_corrections WHERE document_id = $1")
            .bind(doc_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 1, "exactly one OCR correction row expected");

    // Spot-check the row contents.
    let row = sqlx::query(
        r#"SELECT ocr_amount, final_amount, ocr_merchant, final_merchant,
                  ocr_confidence, claim_id, reimbursement_line_id
           FROM ocr_corrections WHERE document_id = $1"#,
    )
    .bind(doc_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    use sqlx::Row;
    let ocr_amount: Option<rust_decimal::Decimal> = row.try_get("ocr_amount").unwrap();
    let final_amount: rust_decimal::Decimal = row.try_get("final_amount").unwrap();
    let ocr_merchant: Option<String> = row.try_get("ocr_merchant").unwrap();
    let final_merchant: String = row.try_get("final_merchant").unwrap();
    let ocr_confidence: Option<f32> = row.try_get("ocr_confidence").unwrap();
    let claim_id_db: Uuid = row.try_get("claim_id").unwrap();
    let line_id_db: Uuid = row.try_get("reimbursement_line_id").unwrap();

    assert_eq!(ocr_amount, Some(rust_decimal::Decimal::new(4250, 2)));
    assert_eq!(final_amount, rust_decimal::Decimal::new(4250, 2));
    assert_eq!(ocr_merchant.as_deref(), Some("COFFEE SHOP"));
    assert_eq!(final_merchant, "COFFEE SHOP");
    assert_eq!(ocr_confidence, Some(85.0));
    assert_eq!(claim_id_db, claim_id);
    assert!(!line_id_db.is_nil(), "reimbursement_line_id must be set");
}

#[tokio::test]
async fn http_ocr_disabled_no_write() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_fb",
            "bob_fb@example.com",
            "correct horse battery staple",
        )
        .await;
    let (ledger_id, doc_id, _owner_id) =
        seed_ocr_apply(&server, "bob_fb@example.com", &cookie).await;
    let claim_id = create_claim(&server, &cookie, ledger_id).await;

    // Opt out of the corpus capture for this test only.
    // SAFETY: tests are single-threaded via `cargo test
    // -- --test-threads=1`, and we restore the previous value at
    // the end.
    let prev = std::env::var("OCR_FEEDBACK").ok();
    std::env::set_var("OCR_FEEDBACK", "false");

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/ocr/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[("claim_id", claim_id.to_string())])
        .send()
        .await
        .expect("apply");

    // Restore the previous env var so other tests are unaffected.
    match prev {
        Some(v) => std::env::set_var("OCR_FEEDBACK", v),
        None => std::env::remove_var("OCR_FEEDBACK"),
    }

    // Apply still succeeds.
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "apply should still redirect when OCR_FEEDBACK=false; got {}",
        resp.status()
    );

    // But the corpus row is NOT written.
    let pool = server.db().pool();
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM ocr_corrections WHERE document_id = $1")
            .bind(doc_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 0, "OCR_FEEDBACK=false must skip the corpus insert");
}

#[tokio::test]
async fn http_ocr_corpus_export_admin_only() {
    let server = TestServer::new().await;

    // Non-admin cookie for the negative case.
    let cookie = server
        .bootstrap_user(
            "carol_fb",
            "carol_fb@example.com",
            "correct horse battery staple",
        )
        .await;
    let (ledger_id, doc_id, _owner_id) =
        seed_ocr_apply(&server, "carol_fb@example.com", &cookie).await;
    let claim_id = create_claim(&server, &cookie, ledger_id).await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/ocr/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[("claim_id", claim_id.to_string())])
        .send()
        .await
        .expect("apply");
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "apply should redirect; got {}",
        resp.status()
    );

    // Non-admin request: 403 (admin middleware blocks it).
    let resp = server
        .client()
        .get(format!("{}/admin/ocr-corpus.json", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("corpus GET as non-admin");
    assert_eq!(
        resp.status(),
        403,
        "non-admin must receive 403; got {}",
        resp.status()
    );

    // Admin user — same session, but flipped to admin role.
    promote_admin(&server, "carol_fb@example.com").await;

    let resp = server
        .client()
        .get(format!("{}/admin/ocr-corpus.json", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("corpus GET as admin");
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert_eq!(status, 200, "admin must receive 200; got {status}");
    assert!(
        ctype.starts_with("application/json"),
        "corpus must be JSON; got {ctype}"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().expect("body is a JSON array");
    assert!(!arr.is_empty(), "corpus should contain at least one row");
    let first = &arr[0];
    // Privacy: no document bytes are ever present.
    assert!(
        first.get("document_id").is_some(),
        "record must have document_id"
    );
    assert!(
        first.get("raw_bytes").is_none() && first.get("bytes").is_none(),
        "record must NOT contain raw document bytes"
    );

    // Anonymous: also blocked. Use a fresh reqwest client (no
    // cookie store) so the cookies from the prior `cookie`
    // session don't leak in.
    let anon_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("anon client");
    let resp = anon_client
        .get(format!("{}/admin/ocr-corpus.json", server.base_url()))
        .send()
        .await
        .expect("corpus GET as anonymous");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status == 303 || status == 307 || status == 401 || status == 403,
        "anonymous must NOT receive 200; got {status} body={body}"
    );
}
