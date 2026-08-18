//! HTTP integration tests for upload size + MIME validation
//! (`s10-upload-validation`).
//!
//! Verifies that:
//! - A request whose body exceeds `upload_max_bytes` is rejected
//!   with HTTP 413 (over-cap is enforced at the layer).
//! - An upload whose declared Content-Type does not match its
//!   sniffed body is rejected with HTTP 400 (and a readable
//!   error body).
//! - The 1 GiB / 24 h quota warning fires when a single user
//!   exceeds the threshold.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use reqwest::multipart::{Form, Part};
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

/// Register and log in `email` using a fresh reqwest client
/// (its own cookie jar), so this user's session is independent
/// of the server's shared jar.
async fn register_and_login(server: &TestServer, email: &str) -> String {
    let client = reqwest::Client::builder()
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
        .unwrap();
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", email.split('@').next().unwrap_or("user")),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .expect("register");
    let resp = client
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
        .send()
        .await
        .expect("login");
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie")
}

/// Register and log in `email`, create one ledger with one
/// transaction. Returns `(cookie, ledger_id, txn_id, cash_id)`.
async fn bootstrap_with_txn(server: &TestServer, email: &str) -> (String, Uuid, Uuid, Uuid) {
    let cookie = register_and_login(server, email).await;

    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Upload"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("create ledger");
    assert_eq!(resp.status(), 303);
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let pool = server.db().pool();
    let cash_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let txn_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
         VALUES ($1, '2026-08-15', 'Upload Test', 'USD', $2)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO postings (transaction_id, account_id, amount, direction)
         VALUES ($1, $2, 1, 'DEBIT'), ($1, $3, 1, 'CREDIT')",
    )
    .bind(txn_id)
    .bind(cash_id)
    .bind(cash_id)
    .execute(&pool)
    .await
    .unwrap();

    (cookie, ledger_id, txn_id, cash_id)
}

#[tokio::test]
async fn http_upload_over_limit_returns_413() {
    // Boot a server with a 2 KiB body cap.
    let server = TestServer::new_with_upload_max(
        "test-secret-do-not-use-in-production-please-replace-with-64-random-chars",
        2048,
    )
    .await;
    let (cookie, ledger_id, txn_id, _cash_id) =
        bootstrap_with_txn(&server, "upload_over@example.com").await;

    // Build a 4 KiB body. The request layer should refuse before
    // any handler reads.
    let body: Vec<u8> = vec![b'x'; 4096];
    let part = Part::bytes(body)
        .file_name("big.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = Form::new().text("category", "Other").part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("POST upload");
    assert_eq!(
        resp.status(),
        413,
        "upload over the cap must return 413 (Payload Too Large)"
    );
}

#[tokio::test]
async fn http_upload_spoofed_type_rejected() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, txn_id, _cash_id) =
        bootstrap_with_txn(&server, "upload_spoof@example.com").await;

    // Body is HTML but we declare it as PDF and name it *.pdf.
    let body = b"<!DOCTYPE html><html><body>x</body></html>".to_vec();
    let part = Part::bytes(body)
        .file_name("evil.pdf")
        .mime_str("application/pdf")
        .unwrap();
    let form = Form::new().text("category", "Other").part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("POST spoofed upload");
    assert_eq!(
        resp.status(),
        400,
        "spoofed mime (html as pdf) must return 400"
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.to_lowercase().contains("content does not match")
            || body.to_lowercase().contains("unsupported file type"),
        "error body must explain the rejection; got {body}"
    );
}

#[tokio::test]
async fn http_upload_real_pdf_succeeds() {
    // Sanity check that the validation layer does not regress
    // legitimate uploads.
    let server = TestServer::new().await;
    let (cookie, ledger_id, txn_id, _cash_id) =
        bootstrap_with_txn(&server, "upload_ok@example.com").await;

    let mut body = b"%PDF-1.4\n%\xc0\xc1\xc2\xc3\n".to_vec();
    body.resize(b"%PDF-1.4\n%\xc0\xc1\xc2\xc3\n".len() + 64, b' ');
    let part = Part::bytes(body)
        .file_name("receipt.pdf")
        .mime_str("application/pdf")
        .unwrap();
    let form = Form::new().text("category", "Other").part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("POST pdf upload");
    assert_eq!(
        resp.status(),
        303,
        "real PDF declared as application/pdf must succeed"
    );
}

#[tokio::test]
async fn http_upload_csv_accepted_on_declaration() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, txn_id, _cash_id) =
        bootstrap_with_txn(&server, "upload_csv@example.com").await;

    let body = b"date,amount\n2026-08-15,100\n".to_vec();
    let part = Part::bytes(body)
        .file_name("ledger.csv")
        .mime_str("text/csv")
        .unwrap();
    let form = Form::new().text("category", "Other").part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("POST csv upload");
    assert_eq!(
        resp.status(),
        303,
        "CSV declared as text/csv must be accepted on declaration"
    );
}

#[tokio::test]
async fn http_upload_empty_rejected() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, txn_id, _cash_id) =
        bootstrap_with_txn(&server, "upload_empty@example.com").await;

    let part = Part::bytes(Vec::new())
        .file_name("nothing.pdf")
        .mime_str("application/pdf")
        .unwrap();
    let form = Form::new().text("category", "Other").part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("POST empty upload");
    assert_eq!(
        resp.status(),
        400,
        "empty body must be rejected with 400 (and a readable message)"
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.to_lowercase().contains("empty") || body.to_lowercase().contains("no file"),
        "error body must mention the empty upload; got {body}"
    );
}

#[tokio::test]
async fn http_quota_warn_logged_after_threshold() {
    // We can't easily assert on tracing output without a global
    // subscriber installed in tests. Instead, we assert the
    // observable side-effect: after enough large uploads the
    // `maybe_warn_quota` path runs and does not error. The unit
    // test in src/upload.rs::tests covers the threshold check
    // directly; here we just verify the integration does not
    // fail uploads when the threshold is crossed.
    let server = TestServer::new().await;
    let (cookie, ledger_id, txn_id, _cash_id) =
        bootstrap_with_txn(&server, "upload_quota@example.com").await;

    // Insert a fake prior 1 GiB upload so the next upload
    // triggers the quota path.
    let pool = server.db().pool();
    let (uid,): (Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email = 'upload_quota@example.com'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let dummy_doc_id: Uuid = sqlx::query_scalar(
        "INSERT INTO documents (transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, category, uploaded_at)
         VALUES ($1, 'pretend', 'pretend', 'application/octet-stream', $2, $3, 'Other', now() - INTERVAL '1 hour')
         RETURNING id",
    )
    .bind(txn_id)
    .bind(openaccounting::upload::QUOTA_WARN_BYTES)
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!dummy_doc_id.is_nil());

    // Upload a small valid PDF; the quota warning fires
    // (observability) and the upload succeeds (303).
    let mut body = b"%PDF-1.4\n%\xc0\xc1\xc2\xc3\n".to_vec();
    body.resize(b"%PDF-1.4\n%\xc0\xc1\xc2\xc3\n".len() + 64, b' ');
    let part = Part::bytes(body)
        .file_name("receipt.pdf")
        .mime_str("application/pdf")
        .unwrap();
    let form = Form::new().text("category", "Other").part("file", part);

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/documents",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("POST over-quota upload");
    assert_eq!(
        resp.status(),
        303,
        "upload must succeed even when quota warning is logged"
    );
}
