//! HTTP integration tests for inline document attach on transaction
//! creation (`a12-transaction-entry-ease`).
//!
//! Covers:
//! - Multipart create with a file → transaction + linked document.
//! - Multipart create without a file → transaction only.
//! - The inline-attached document is subject to the existing
//!   document authorization rules (cross-user download → 404).
//! - The legacy urlencoded create path still works (regression).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
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

/// Register + log in, create a ledger, and return the client, the
/// ledger id, and two account ids (an EXPENSE debit account and an
/// ASSET credit account).
async fn setup(server: &TestServer, tag: &str) -> (reqwest::Client, Uuid, Uuid, Uuid) {
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

    let pool = server.db().pool();
    let debit: (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'EXPENSE' LIMIT 1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let credit: (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' LIMIT 1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    (client, ledger_id, debit.0, credit.0)
}

fn base_form(ledger_id: Uuid, debit: Uuid, credit: Uuid, desc: &str) -> reqwest::multipart::Form {
    reqwest::multipart::Form::new()
        .text("date", "2026-08-20")
        .text("description", desc.to_string())
        .text("action", "save")
        .text("lines[0][account_id]", debit.to_string())
        .text("lines[0][direction]", "DEBIT")
        .text("lines[0][amount]", "1.00")
        .text("lines[1][account_id]", credit.to_string())
        .text("lines[1][direction]", "CREDIT")
        .text("lines[1][amount]", "1.00")
}

#[tokio::test]
async fn http_inline_document_attached_on_create() {
    let server = TestServer::new().await;
    let (client, ledger_id, debit, credit) = setup(&server, "inline-doc").await;

    let form = base_form(ledger_id, debit, credit, "inline doc test").part(
        "files",
        reqwest::multipart::Part::bytes(b"receipt bytes")
            .file_name("receipt.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "multipart create must redirect");

    let pool = server.db().pool();
    let txn: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM transactions WHERE description = 'inline doc test'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    let txn_id = txn.expect("transaction created").0;
    let doc: (String,) = sqlx::query_as("SELECT filename FROM documents WHERE transaction_id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .expect("document linked to the transaction");
    assert_eq!(doc.0, "receipt.txt");
}

#[tokio::test]
async fn http_inline_create_without_document() {
    let server = TestServer::new().await;
    let (client, ledger_id, debit, credit) = setup(&server, "inline-nodoc").await;

    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .multipart(base_form(ledger_id, debit, credit, "inline no-doc test"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        303,
        "multipart create without files still works"
    );

    let pool = server.db().pool();
    let txn: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM transactions WHERE description = 'inline no-doc test'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    let txn_id = txn.expect("transaction created").0;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM documents WHERE transaction_id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "no documents should be linked");
}

#[tokio::test]
async fn http_inline_document_authorization() {
    let server = TestServer::new().await;
    let (owner, ledger_id, debit, credit) = setup(&server, "inline-auth-owner").await;

    let form = base_form(ledger_id, debit, credit, "inline auth test").part(
        "files",
        reqwest::multipart::Part::bytes(b"secret receipt")
            .file_name("secret.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    owner
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();

    let pool = server.db().pool();
    let doc: (Uuid,) = sqlx::query_as(
        "SELECT d.id FROM documents d
         JOIN transactions t ON t.id = d.transaction_id
         WHERE t.description = 'inline auth test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // A different user (not a member of the ledger) must not be
    // able to download the inline-attached document.
    let (intruder, _) = {
        let c = make_client();
        let email = "inline-intruder@example.com";
        c.post(format!("{}/register", server.base_url()))
            .form(&[
                ("email", email),
                ("username", "inline-intruder"),
                ("password", PASSWORD),
                ("password_confirm", PASSWORD),
            ])
            .send()
            .await
            .unwrap();
        c.post(format!("{}/login", server.base_url()))
            .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
            .send()
            .await
            .unwrap();
        (c, ())
    };
    let resp = intruder
        .get(format!(
            "{}/ledgers/{ledger_id}/documents/{}/download",
            server.base_url(),
            doc.0
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        404,
        "non-member must get 404 on an inline-attached document"
    );
}

#[tokio::test]
async fn http_urlencoded_create_still_works() {
    let server = TestServer::new().await;
    let (client, ledger_id, debit, credit) = setup(&server, "inline-urlenc").await;

    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "urlencoded still works"),
            ("action", "save"),
            ("lines[0][account_id]", debit.to_string().as_str()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "1.00"),
            ("lines[1][account_id]", credit.to_string().as_str()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "1.00"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        303,
        "urlencoded create path must keep working"
    );
}

#[tokio::test]
async fn http_multileg_editor_is_default() {
    let server = TestServer::new().await;
    let (client, ledger_id, _, _) = setup(&server, "multileg").await;

    let resp = client
        .get(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains(r#"id="advanced-entry">"#),
        "the multi-leg editor must be visible by default"
    );
    assert!(
        body.contains(r#"id="simple-entry" hidden"#),
        "the two-leg simple view must be hidden by default"
    );
}

#[tokio::test]
async fn http_upload_unbound_document() {
    let server = TestServer::new().await;
    let (client, ledger_id, _, _) = setup(&server, "inbox-upload").await;

    let form = reqwest::multipart::Form::new().part(
        "files",
        reqwest::multipart::Part::bytes(b"inbox receipt")
            .file_name("inbox-receipt.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/documents",
            server.base_url()
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "unbound upload must redirect");

    let pool = server.db().pool();
    let doc: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT transaction_id, ledger_id FROM documents WHERE filename = 'inbox-receipt.txt'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(doc.0, None, "unbound document has no transaction");
    assert_eq!(
        doc.1,
        Some(ledger_id),
        "unbound document anchored to the ledger"
    );
}

#[tokio::test]
async fn http_upload_unbound_non_writer_403() {
    let server = TestServer::new().await;
    let (_, ledger_id, _, _) = setup(&server, "inbox-owner").await;

    // A non-member cannot upload to the ledger.
    let intruder = make_client();
    let email = "inbox-intruder@example.com";
    intruder
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "inbox-intruder"),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    intruder
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
        .send()
        .await
        .unwrap();

    let form = reqwest::multipart::Form::new().part(
        "files",
        reqwest::multipart::Part::bytes(b"nope")
            .file_name("x.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let resp = intruder
        .post(format!(
            "{}/ledgers/{ledger_id}/documents",
            server.base_url()
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert!(
        resp.status() == 403 || resp.status() == 404,
        "non-writer must be blocked from uploading (got {})",
        resp.status()
    );
}

#[tokio::test]
async fn http_inbox_lists_unbound() {
    let server = TestServer::new().await;
    let (client, ledger_id, _, _) = setup(&server, "inbox-list").await;

    let form = reqwest::multipart::Form::new().part(
        "files",
        reqwest::multipart::Part::bytes(b"list me")
            .file_name("list-me.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    client
        .post(format!(
            "{}/ledgers/{ledger_id}/documents",
            server.base_url()
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!(
            "{}/ledgers/{ledger_id}/documents",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("list-me.txt"), "unbound document listed");
    assert!(body.contains("Unbound"), "unbound state shown");
    assert!(
        body.contains("Bind to transaction"),
        "bind action present for unbound documents"
    );
}

async fn upload_unbound(
    server: &TestServer,
    client: &reqwest::Client,
    ledger_id: Uuid,
    name: &str,
) -> Uuid {
    let name = name.to_string();
    let form = reqwest::multipart::Form::new().part(
        "files",
        reqwest::multipart::Part::bytes(format!("{name} bytes").into_bytes())
            .file_name(name.clone())
            .mime_str("text/plain")
            .unwrap(),
    );
    client
        .post(format!(
            "{}/ledgers/{ledger_id}/documents",
            server.base_url()
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let pool = server.db().pool();
    let id: (Uuid,) = sqlx::query_as("SELECT id FROM documents WHERE filename = $1")
        .bind(name)
        .fetch_one(&pool)
        .await
        .unwrap();
    id.0
}

async fn create_txn(
    client: &reqwest::Client,
    server: &TestServer,
    ledger_id: Uuid,
    debit: Uuid,
    credit: Uuid,
    desc: &str,
) -> Uuid {
    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .form(&[
            ("date", "2026-08-20"),
            ("description", desc),
            ("action", "save"),
            ("lines[0][account_id]", debit.to_string().as_str()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "2.00"),
            ("lines[1][account_id]", credit.to_string().as_str()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "2.00"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "create must redirect");
    let pool = server.db().pool();
    let id: (Uuid,) = sqlx::query_as("SELECT id FROM transactions WHERE description = $1")
        .bind(desc)
        .fetch_one(&pool)
        .await
        .unwrap();
    id.0
}

#[tokio::test]
async fn http_bind_document_to_transaction() {
    let server = TestServer::new().await;
    let (client, ledger_id, debit, credit) = setup(&server, "bind-to-txn").await;

    let doc_id = upload_unbound(&server, &client, ledger_id, "bind-me.txt").await;
    let txn_id = create_txn(&client, &server, ledger_id, debit, credit, "bind target").await;

    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/bind",
            server.base_url()
        ))
        .form(&[("transaction_id", txn_id.to_string())])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "bind must redirect");

    let bound: (Option<Uuid>,) =
        sqlx::query_as("SELECT transaction_id FROM documents WHERE id = $1")
            .bind(doc_id)
            .fetch_one(&server.db().pool())
            .await
            .unwrap();
    assert_eq!(bound.0, Some(txn_id), "document bound to the transaction");

    let audit: Option<(Uuid,)> = sqlx::query_as(
        "SELECT entity_id FROM audit_entries WHERE action = 'bind' AND entity_type = 'document' AND entity_id = $1",
    )
    .bind(doc_id)
    .fetch_optional(&server.db().pool())
    .await
    .unwrap();
    assert!(audit.is_some(), "bind must be audit-logged");
}

#[tokio::test]
async fn http_bind_by_creating_transaction() {
    let server = TestServer::new().await;
    let (client, ledger_id, debit, credit) = setup(&server, "bind-create").await;

    let doc_id = upload_unbound(&server, &client, ledger_id, "create-and-bind.txt").await;

    // Open the new-transaction form with ?bind_doc, then create.
    let form = client
        .get(format!(
            "{}/ledgers/{ledger_id}/transactions/new?bind_doc={doc_id}",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    let html = form.text().await.unwrap();
    assert!(
        html.contains("bind_doc"),
        "form must carry the bind_doc hint"
    );

    // Create the transaction with the bind_doc hidden field.
    let resp = client
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "create and bind"),
            ("action", "save"),
            ("bind_doc", doc_id.to_string().as_str()),
            ("lines[0][account_id]", debit.to_string().as_str()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "2.00"),
            ("lines[1][account_id]", credit.to_string().as_str()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "2.00"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "create with bind_doc must redirect");
    let bound: (Option<Uuid>,) =
        sqlx::query_as("SELECT transaction_id FROM documents WHERE id = $1")
            .bind(doc_id)
            .fetch_one(&server.db().pool())
            .await
            .unwrap();
    assert!(
        bound.0.is_some(),
        "new transaction bound to the unbound document"
    );
}

#[tokio::test]
async fn http_bind_requires_writer() {
    let server = TestServer::new().await;
    let (client, ledger_id, debit, credit) = setup(&server, "bind-writer").await;
    let doc_id = upload_unbound(&server, &client, ledger_id, "bind-guard.txt").await;
    let txn_id = create_txn(&client, &server, ledger_id, debit, credit, "bind guard txn").await;

    let intruder = make_client();
    let email = "bind-intruder@example.com";
    intruder
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "bind-intruder"),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    intruder
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
        .send()
        .await
        .unwrap();

    let resp = intruder
        .post(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/bind",
            server.base_url()
        ))
        .form(&[("transaction_id", txn_id.to_string())])
        .send()
        .await
        .unwrap();
    assert!(
        resp.status() == 403 || resp.status() == 404,
        "non-writer must be blocked from binding (got {})",
        resp.status()
    );
    let still: (Option<Uuid>,) =
        sqlx::query_as("SELECT transaction_id FROM documents WHERE id = $1")
            .bind(doc_id)
            .fetch_one(&server.db().pool())
            .await
            .unwrap();
    assert_eq!(still.0, None, "blocked bind must not change the document");
}

#[tokio::test]
async fn http_unbound_document_download_authorization() {
    let server = TestServer::new().await;
    let (client, ledger_id, _, _) = setup(&server, "unbound-dl").await;
    let doc_id = upload_unbound(&server, &client, ledger_id, "unbound-dl.txt").await;

    // Writer can download.
    let writer = client
        .get(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/download",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        writer.status(),
        200,
        "writer can download an unbound document"
    );
    let body = writer.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"unbound-dl.txt bytes");

    // Non-member gets 404.
    let intruder = make_client();
    let email = "unbound-intruder@example.com";
    intruder
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "unbound-intruder"),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    intruder
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
        .send()
        .await
        .unwrap();
    let resp = intruder
        .get(format!(
            "{}/ledgers/{ledger_id}/documents/{doc_id}/download",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        404,
        "non-member cannot download an unbound document"
    );
}
