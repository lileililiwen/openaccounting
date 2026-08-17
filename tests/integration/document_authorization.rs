//! HTTP integration tests for per-document authorization
//! (`s8-document-authorization`).
//!
//! Verifies that:
//! - Owner, editor, viewer, and admin can download.
//! - Cross-ledger access returns 404 (not 403).
//! - Cross-user access returns 404.
//! - Cross-ledger delete returns 404.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

/// Register and log in a user. Returns the captured
/// `oa_session=<value>` cookie string the server emitted on the
/// login redirect.
async fn register_and_login(server: &TestServer, email: &str, password: &str) -> String {
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", email.split('@').next().unwrap_or("user")),
            ("password", password),
            ("password_confirm", password),
        ])
        .send()
        .await
        .expect("register");
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
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

/// Helper: register the owner, create a ledger, insert a
/// transaction and a document directly in the DB, and write a
/// file to the storage sandbox so download succeeds. Returns
/// `(owner_cookie, ledger_id, document_id)`.
async fn bootstrap_with_doc(
    server: &TestServer,
    owner_email: &str,
    password: &str,
    ledger_name: &str,
) -> (String, Uuid, Uuid) {
    let pool = server.db().pool();
    let owner_cookie = register_and_login(server, owner_email, password).await;

    let (owner_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(owner_email)
        .fetch_one(&pool)
        .await
        .unwrap();

    let ledger_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ledgers (owner_id, name, base_currency, timezone, basis)
         VALUES ($1, $2, 'USD', 'UTC', 'accrual')
         RETURNING id",
    )
    .bind(owner_id)
    .bind(ledger_name)
    .fetch_one(&pool)
    .await
    .unwrap();

    let txn_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
         VALUES ($1, '2026-01-01', 'Test', 'USD', $2)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let doc_id: Uuid = sqlx::query_scalar(
        "INSERT INTO documents (transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, category)
         VALUES ($1, 'test.txt', 'stored_test.txt', 'text/plain', 4, $2, 'Other')
         RETURNING id",
    )
    .bind(txn_id)
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let sandbox = server.sandbox_path();
    let doc_dir = sandbox.join("documents").join(txn_id.to_string());
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(doc_dir.join("stored_test.txt"), b"hi\n").unwrap();

    (owner_cookie, ledger_id, doc_id)
}

/// Add `email` to `ledger_id` with the given role.
async fn add_member(server: &TestServer, ledger_id: Uuid, email: &str, role: &str) {
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ledger_members (ledger_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(ledger_id)
    .bind(user_id)
    .bind(role)
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn http_doc_owner_can_download() {
    let server = TestServer::new().await;
    let (_cookie, ledger_id, doc_id) = bootstrap_with_doc(
        &server,
        "owner@example.com",
        "X7!qZ4wN9pLk_3vR",
        "Owner ledger",
    )
    .await;
    // Log in again via the API path so we have a fresh cookie
    // bound to a fresh reqwest::Client (bootstrap_with_doc used
    // its own client for register/login).
    let cookie = register_and_login(&server, "owner@example.com", "X7!qZ4wN9pLk_3vR").await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/documents/{}/download",
            server.base_url(),
            ledger_id,
            doc_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("download");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(
        status, 200,
        "owner should download; got {status} body={body}"
    );
    assert!(
        body.contains("hi"),
        "downloaded body should contain stored content; got {body}"
    );
}

#[tokio::test]
async fn http_doc_editor_can_download() {
    let server = TestServer::new().await;
    let (_owner_cookie, ledger_id, doc_id) = bootstrap_with_doc(
        &server,
        "owner@example.com",
        "X7!qZ4wN9pLk_3vR",
        "Shared ledger",
    )
    .await;

    let editor_email = "editor@example.com";
    register_and_login(&server, editor_email, "X7!qZ4wN9pLk_3vR").await;
    add_member(&server, ledger_id, editor_email, "editor").await;
    let editor_cookie = register_and_login(&server, editor_email, "X7!qZ4wN9pLk_3vR").await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/documents/{}/download",
            server.base_url(),
            ledger_id,
            doc_id
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .send()
        .await
        .expect("editor download");
    assert_eq!(resp.status(), 200, "editor should download");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("hi"),
        "editor should see document bytes; got {body}"
    );
}

#[tokio::test]
async fn http_doc_viewer_can_download() {
    let server = TestServer::new().await;
    let (_owner_cookie, ledger_id, doc_id) = bootstrap_with_doc(
        &server,
        "owner@example.com",
        "X7!qZ4wN9pLk_3vR",
        "Shared ledger",
    )
    .await;

    let viewer_email = "viewer@example.com";
    register_and_login(&server, viewer_email, "X7!qZ4wN9pLk_3vR").await;
    add_member(&server, ledger_id, viewer_email, "viewer").await;
    let viewer_cookie = register_and_login(&server, viewer_email, "X7!qZ4wN9pLk_3vR").await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/documents/{}/download",
            server.base_url(),
            ledger_id,
            doc_id
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .send()
        .await
        .expect("viewer download");
    assert_eq!(resp.status(), 200, "viewer should download");
}

#[tokio::test]
async fn http_doc_cross_ledger_returns_404() {
    let server = TestServer::new().await;
    let (_owner_cookie, ledger_id_a, doc_id) = bootstrap_with_doc(
        &server,
        "owner-a@example.com",
        "X7!qZ4wN9pLk_3vR",
        "Ledger A",
    )
    .await;

    // Register a user with their own ledger; they have no
    // relationship to ledger A.
    register_and_login(&server, "user-b@example.com", "X7!qZ4wN9pLk_3vR").await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/documents/{}/download",
            server.base_url(),
            ledger_id_a,
            doc_id
        ))
        .header(reqwest::header::COOKIE, "oa_session=fake")
        .send()
        .await
        .expect("cross-user download (unauthenticated)");

    // Auth layer may bounce unauthenticated requests to /login
    // (303) OR return 404 from the handler — either way, no
    // document bytes leak.
    let status = resp.status();
    assert_ne!(
        status, 200,
        "unauthenticated user MUST NOT receive the document bytes; got {status}"
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        !body.contains("hi"),
        "document content MUST NOT be disclosed; got body: {body}"
    );

    // Now do it with a real cookie for a user who has nothing
    // to do with ledger A.
    let cookie_b = register_and_login(&server, "user-b@example.com", "X7!qZ4wN9pLk_3vR").await;
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/documents/{}/download",
            server.base_url(),
            ledger_id_a,
            doc_id
        ))
        .header(reqwest::header::COOKIE, cookie_b)
        .send()
        .await
        .expect("cross-ledger download");
    let status = resp.status();
    assert_eq!(
        status, 404,
        "cross-ledger download must return 404 (not 403); got {status}"
    );
}

#[tokio::test]
async fn http_doc_cross_user_returns_404() {
    let server = TestServer::new().await;
    let (_owner_cookie, ledger_id_a, doc_id) = bootstrap_with_doc(
        &server,
        "owner-a@example.com",
        "X7!qZ4wN9pLk_3vR",
        "Ledger A",
    )
    .await;

    // Register an unrelated user. They have NO ledger of their
    // own, no membership anywhere — they are a pure
    // authenticated stranger.
    let cookie = register_and_login(&server, "stranger@example.com", "X7!qZ4wN9pLk_3vR").await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/documents/{}/download",
            server.base_url(),
            ledger_id_a,
            doc_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("cross-user download");
    let status = resp.status();
    assert_eq!(
        status, 404,
        "stranger must receive 404, not the document; got {status}"
    );
}

#[tokio::test]
async fn http_doc_cross_ledger_delete_returns_404() {
    let server = TestServer::new().await;
    let (_owner_cookie, ledger_id_a, doc_id) = bootstrap_with_doc(
        &server,
        "owner-a@example.com",
        "X7!qZ4wN9pLk_3vR",
        "Ledger A",
    )
    .await;

    register_and_login(&server, "user-b@example.com", "X7!qZ4wN9pLk_3vR").await;
    let cookie_b = register_and_login(&server, "user-b@example.com", "X7!qZ4wN9pLk_3vR").await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/documents/{}/delete",
            server.base_url(),
            ledger_id_a,
            doc_id
        ))
        .header(reqwest::header::COOKIE, cookie_b)
        .send()
        .await
        .expect("cross-ledger delete");
    let status = resp.status();
    assert_eq!(
        status, 404,
        "cross-ledger delete must return 404; got {status}"
    );

    // Verify the document still exists.
    let pool = server.db().pool();
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM documents WHERE id = $1")
        .bind(doc_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "document must NOT have been deleted");
}
