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
