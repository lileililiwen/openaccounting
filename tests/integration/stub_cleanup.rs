//! HTTP integration tests for the stub cleanup (`a19-stub-cleanup`):
//! API token UI, inter-ledger transfer form + create.
//! (Plaid signature verification is unit-tested in bank_feeds.rs.)

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

async fn register_and_login(server: &TestServer, tag: &str) -> reqwest::Client {
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
    client
}

async fn create_ledger(client: &reqwest::Client, server: &TestServer, name: &str) -> Uuid {
    let resp = client
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", name.to_string()),
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
    loc.rsplit('/').next().unwrap().parse().unwrap()
}

#[tokio::test]
async fn api_tokens_page_issue_and_revoke() {
    let server = TestServer::new().await;
    let client = register_and_login(&server, "tokens").await;
    let base = server.base_url();
    let pool = server.db().pool();

    // List page renders.
    let body = client
        .get(format!("{base}/account/api-tokens"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("API"), "token page should render");

    // Issue a token — the plaintext is shown once.
    let resp = client
        .post(format!("{base}/account/api-tokens/create"))
        .form(&[("name", "CI token")])
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("CI token") && body.contains("oa_"),
        "plaintext token should be shown once, got: {}",
        body.chars().take(300).collect::<String>()
    );

    let (token_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM api_tokens WHERE name = 'CI token' AND revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Revoke.
    client
        .post(format!("{base}/account/api-tokens/{token_id}/revoke"))
        .send()
        .await
        .unwrap();
    let revoked: bool = sqlx::query_scalar(
        "SELECT revoked_at IS NOT NULL FROM api_tokens WHERE id = $1",
    )
    .bind(token_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(revoked, "token should be revoked");
}

#[tokio::test]
async fn transfer_form_renders_ledgers() {
    let server = TestServer::new().await;
    let client = register_and_login(&server, "trform").await;
    let base = server.base_url();
    create_ledger(&client, &server, "Ledger One").await;
    create_ledger(&client, &server, "Ledger Two").await;

    let body = client
        .get(format!("{base}/transfers/inter-ledger"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("Ledger One")
            && body.contains("Ledger Two")
            && body.contains("Inter-ledger transfer"),
        "transfer form should list ledgers, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn transfer_creates_two_transactions_and_link() {
    let server = TestServer::new().await;
    let client = register_and_login(&server, "trcreate").await;
    let base = server.base_url();
    let pool = server.db().pool();

    let from_ledger = create_ledger(&client, &server, "Transfer A").await;
    let to_ledger = create_ledger(&client, &server, "Transfer B").await;

    let from_account: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'")
            .bind(from_ledger)
            .fetch_one(&pool)
            .await
            .unwrap();
    let to_account: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'")
            .bind(to_ledger)
            .fetch_one(&pool)
            .await
            .unwrap();

    let resp = client
        .post(format!("{base}/transfers/inter-ledger"))
        .form(&[
            ("from_ledger_id", from_ledger.to_string().as_str()),
            ("to_ledger_id", to_ledger.to_string().as_str()),
            ("from_account_id", from_account.to_string().as_str()),
            ("to_account_id", to_account.to_string().as_str()),
            ("amount", "250.00"),
            ("date", "2026-08-20"),
            ("description", "Move cash between books"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "transfer should redirect on success");

    let link_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM inter_ledger_transfers WHERE from_ledger_id = $1 AND to_ledger_id = $2",
    )
    .bind(from_ledger)
    .bind(to_ledger)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link_count.0, 1, "one inter-ledger link expected");

    let (from_txns, to_txns): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM transactions WHERE ledger_id = $1),
                (SELECT COUNT(*) FROM transactions WHERE ledger_id = $2)",
    )
    .bind(from_ledger)
    .bind(to_ledger)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(from_txns, 1, "source ledger should have one transaction");
    assert_eq!(to_txns, 1, "target ledger should have one transaction");
}
