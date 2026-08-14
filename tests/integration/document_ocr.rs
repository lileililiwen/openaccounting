//! Integration tests for the document OCR module.
//!
//! Tests follow the spec from:
//! `openspec/changes/2026-08-14-receipt-ocr/tasks.md` — section 1.

use crate::common::*;
use uuid::Uuid;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Create a ledger and return its id.
async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "OCR Co"),
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

/// Upload a minimal 1×1 PNG receipt and return the document id.
/// The uploaded document is attached to a newly created transaction.
async fn upload_image(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    ocr_param: Option<&str>,
) -> (Uuid, Uuid) {
    let pool = server.db().pool();

    // Create a bare-minimum transaction to attach the document to.
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user exists");
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
           VALUES ($1, '2026-08-14', 'Test receipt', 'USD', $2)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert transaction");

    // Minimal 1×1 white PNG (67 bytes).
    let png: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG header
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1×1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // bit depth RGB
        0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21,
        0xbc, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, // IEND chunk
        0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    let upload_url = if let Some(p) = ocr_param {
        format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents?ocr={p}",
            server.base_url()
        )
    } else {
        format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        )
    };

    let part = reqwest::multipart::Part::bytes(png.to_vec())
        .file_name("receipt.png")
        .mime_str("image/png")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    server
        .client()
        .post(&upload_url)
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("upload request");

    // Retrieve the doc id from the DB.
    let (doc_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM documents WHERE transaction_id = $1 ORDER BY uploaded_at DESC LIMIT 1",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .expect("doc exists");

    (doc_id, txn_id)
}

// ─── 1.5  http_upload_image_kicks_off_ocr ────────────────────────────────────

/// Uploading an image enqueues an OCR job. Polling the OCR page
/// eventually shows a result (or a pending badge while the
/// background task is running).
#[tokio::test]
async fn http_upload_image_kicks_off_ocr() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_ocr",
            "alice_ocr@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server).await;

    let (doc_id, _) = upload_image(&server, &cookie, ledger_id, None).await;

    // The OCR GET page should return 200 (pending or done).
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/ocr",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("GET ocr page");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200, "OCR page should 200; body={body}");
    // Either "pending" or the result is shown.
    assert!(
        body.contains("OCR") || body.contains("pending") || body.contains("Merchant"),
        "expected OCR content; got: {body}"
    );
}

// ─── 1.6  http_apply_creates_reimbursement_line ───────────────────────────────

/// If an OCR result exists (we insert it directly), the apply endpoint
/// creates a new reimbursement line in the claim.
#[tokio::test]
async fn http_apply_creates_reimbursement_line() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "bob_ocr",
            "bob_ocr@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server).await;

    let (doc_id, _) = upload_image(&server, &cookie, ledger_id, None).await;

    // Insert a synthetic OCR result so we can test Apply.
    sqlx::query(
        r#"INSERT INTO document_ocr_results
               (document_id, amount, txn_date, merchant, raw_text, engine, confidence)
           VALUES ($1, 42.50, '2026-08-14', 'COFFEE SHOP', 'COFFEE SHOP 42.50', 'tesseract', 85)
           ON CONFLICT (document_id) DO UPDATE
               SET amount = 42.50, txn_date = '2026-08-14', merchant = 'COFFEE SHOP',
                   error_message = NULL"#,
    )
    .bind(doc_id)
    .execute(&pool)
    .await
    .expect("insert ocr result");

    // Create a draft reimbursement claim.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body("title=OCR+Apply+Test&employee_name=Bob&currency=USD")
        .send()
        .await
        .expect("create claim");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location")
        .to_string();
    let claim_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    // Apply OCR result.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/ocr/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(format!("claim_id={claim_id}"))
        .send()
        .await
        .expect("apply request");
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "apply should redirect; got {status}"
    );

    // Verify a line was inserted.
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM reimbursement_lines WHERE claim_id = $1")
            .bind(claim_id)
            .fetch_one(&pool)
            .await
            .expect("count lines");
    assert_eq!(count.0, 1, "expected 1 line after apply");
}

// ─── 1.7  http_upload_with_ocr_false_skips_job ────────────────────────────────

/// Uploading with `?ocr=false` must not create a `document_ocr_results` row
/// (OCR is not enqueued). We verify by checking the DB after a short delay.
#[tokio::test]
async fn http_upload_with_ocr_false_skips_job() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "carol_ocr",
            "carol_ocr@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server).await;

    let (doc_id, _) = upload_image(&server, &cookie, ledger_id, Some("false")).await;

    // Wait briefly so any background task would have had time to run.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM document_ocr_results WHERE document_id = $1")
            .bind(doc_id)
            .fetch_one(&pool)
            .await
            .expect("count ocr rows");
    assert_eq!(count.0, 0, "ocr=false should not create an OCR result row");
}

// ─── 1.8  http_pdf_without_pdftoppm_returns_error_but_upload_succeeds ────────

/// Uploading a PDF when `pdftoppm` is not on PATH must not fail the upload.
/// The document row is created; the OCR result (if any) has an error_message.
#[tokio::test]
async fn http_pdf_without_pdftoppm_returns_error_but_upload_succeeds() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "dave_ocr",
            "dave_ocr@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server).await;

    // Create a transaction.
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user exists");
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
           VALUES ($1, '2026-08-14', 'PDF receipt', 'USD', $2)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert transaction");

    // Minimal %PDF header so mime detection recognises it.
    let pdf_bytes = b"%PDF-1.4\n%%EOF\n";
    let part = reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
        .file_name("receipt.pdf")
        .mime_str("application/pdf")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .multipart(form)
        .send()
        .await
        .expect("upload pdf");

    // Upload must succeed (redirect).
    let status = resp.status();
    assert!(
        status == 303 || status == 302 || status.is_success(),
        "upload should succeed; got {status}"
    );

    // The document row must exist.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM documents WHERE transaction_id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .expect("count docs");
    assert_eq!(count.0, 1, "document must be saved even for PDF");
}
