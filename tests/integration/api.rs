//! HTTP integration tests for the REST API (`a1-rest-api`).
//!
//! Covers:
//! - Bearer-token requirement (401 without auth, 200 with).
//! - Create a balanced transaction → 201 + RFC 7807 on errors.
//! - Idempotency-Key replay.
//! - Trial-balance endpoint.
//! - Token revocation invalidates subsequent requests.
//!
//! Rate-limit (task 1.5) is deferred; documented in tasks.md.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use serde_json::json;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

/// Register a user via the HTTP form, then promote them and
/// issue an API token directly in the DB. Returns the
/// `Bearer oa_live_…` plaintext so the test can put it in
/// the `Authorization` header.
async fn bootstrap_with_token(server: &TestServer, email: &str) -> (String, Uuid) {
    server.bootstrap_user(email, email, PASSWORD).await;
    let pool = server.db().pool();
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let issued = openaccounting::auth::api_token::issue_token(&pool, user_id, "test-token")
        .await
        .expect("issue token");
    (issued.plaintext, user_id)
}

#[tokio::test]
async fn http_api_token_required() {
    let server = TestServer::new().await;
    let (ledger_cookie, _owner_id) = bootstrap_with_token(&server, "api-token@example.com").await;

    // No Authorization header → 401.
    let resp = server
        .client()
        .get(format!("{}/api/v1/ledgers", server.base_url()))
        .send()
        .await
        .expect("GET ledgers");
    assert_eq!(
        resp.status(),
        401,
        "missing bearer must return 401; got {}",
        resp.status()
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["title"]
            .as_str()
            .unwrap_or("")
            .contains("Unauthorized"),
        "401 body must be RFC 7807 problem-details; got {body}"
    );
    assert_eq!(body["status"].as_u64(), Some(401));

    // Wrong token → 401.
    let resp = server
        .client()
        .get(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, "Bearer oa_live_wrong")
        .send()
        .await
        .expect("GET ledgers");
    assert_eq!(resp.status(), 401);

    // Correct token → 200 (empty list, since no ledgers created yet).
    let resp = server
        .client()
        .get(format!("{}/api/v1/ledgers", server.base_url()))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {ledger_cookie}"),
        )
        .send()
        .await
        .expect("GET ledgers");
    assert_eq!(resp.status(), 200, "valid bearer must succeed");
}

#[tokio::test]
async fn http_api_create_ledger_and_transaction() {
    let server = TestServer::new().await;
    let (token, _uid) = bootstrap_with_token(&server, "api-create@example.com").await;

    // Create a ledger via the API.
    let resp = server
        .client()
        .post(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({
            "name": "API Co",
            "base_currency": "USD",
            "timezone": "UTC",
            "basis": "accrual",
        }))
        .send()
        .await
        .expect("POST /api/v1/ledgers");
    assert_eq!(resp.status(), 201);
    let ledger: serde_json::Value = resp.json().await.unwrap();
    let ledger_id = ledger["id"].as_str().unwrap();
    assert!(!ledger_id.is_empty());

    // Seed default accounts (no /api/v1/accounts/seed — use the
    // migration's default chart via the seed_accounts SQL).
    let pool = server.db().pool();
    sqlx::query(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Cash', 'ASSET', 'CURRENT_ASSET', 'USD'),
                ($1, 'Sales', 'INCOME', 'OPERATING_INCOME', 'USD')
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::parse_str(ledger_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let cash: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash' LIMIT 1",
    )
    .bind(Uuid::parse_str(ledger_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    let sales: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales' LIMIT 1",
    )
    .bind(Uuid::parse_str(ledger_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();

    // POST a balanced transaction → 201.
    let resp = server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({
            "date": "2026-08-15",
            "description": "API sale",
            "lines": [
                { "account_id": cash.to_string(), "direction": "DEBIT", "amount": "100.00" },
                { "account_id": sales.to_string(), "direction": "CREDIT", "amount": "100.00" }
            ]
        }))
        .send()
        .await
        .expect("POST transaction");
    let status = resp.status();
    let body_text = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 201,
        "balanced transaction must return 201; got {status} body={body_text}"
    );
    let body: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(body["total"].as_str(), Some("100.00"));
    let txn_id = body["id"].as_str().unwrap();
    assert!(!txn_id.is_empty());

    // GET the transaction → 200.
    let resp = server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions/{txn_id}",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .expect("GET transaction");
    assert_eq!(resp.status(), 200);

    // GET the transaction list → 200.
    let resp = server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .expect("GET transactions");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn http_api_create_transaction_unbalanced_returns_problem_details() {
    let server = TestServer::new().await;
    let (token, _uid) = bootstrap_with_token(&server, "api-unbal@example.com").await;

    // Create a ledger + accounts.
    let resp = server
        .client()
        .post(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({
            "name": "Unbal Co",
            "base_currency": "USD",
        }))
        .send()
        .await
        .unwrap();
    let ledger: serde_json::Value = resp.json().await.unwrap();
    let ledger_id = ledger["id"].as_str().unwrap();
    let pool = server.db().pool();
    let luid = Uuid::parse_str(ledger_id).unwrap();
    sqlx::query(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Cash', 'ASSET', 'CURRENT_ASSET', 'USD'),
                ($1, 'Sales', 'INCOME', 'OPERATING_INCOME', 'USD')",
    )
    .bind(luid)
    .execute(&pool)
    .await
    .unwrap();
    let cash: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash'")
            .bind(luid)
            .fetch_one(&pool)
            .await
            .unwrap();
    let sales: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales'")
            .bind(luid)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Unbalanced: 100 debit, 50 credit.
    let resp = server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({
            "date": "2026-08-15",
            "description": "Unbalanced test",
            "lines": [
                { "account_id": cash.to_string(), "direction": "DEBIT", "amount": "100.00" },
                { "account_id": sales.to_string(), "direction": "CREDIT", "amount": "50.00" }
            ]
        }))
        .send()
        .await
        .expect("POST unbalanced");
    assert_eq!(
        resp.status(),
        400,
        "unbalanced transaction must return 400; got {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/problem+json"),
        "400 must carry problem+json content-type; got {ct}"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"].as_u64(), Some(400));
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains("do not balance"),
        "detail must explain the imbalance; got {body}"
    );
}

#[tokio::test]
async fn http_api_idempotency_replay_returns_same() {
    let server = TestServer::new().await;
    let (token, _uid) = bootstrap_with_token(&server, "api-idem@example.com").await;

    // Ledger + accounts.
    let resp = server
        .client()
        .post(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({ "name": "Idem Co", "base_currency": "USD" }))
        .send()
        .await
        .unwrap();
    let ledger: serde_json::Value = resp.json().await.unwrap();
    let ledger_id = ledger["id"].as_str().unwrap();
    let pool = server.db().pool();
    let luid = Uuid::parse_str(ledger_id).unwrap();
    sqlx::query(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Cash', 'ASSET', 'CURRENT_ASSET', 'USD'),
                ($1, 'Sales', 'INCOME', 'OPERATING_INCOME', 'USD')",
    )
    .bind(luid)
    .execute(&pool)
    .await
    .unwrap();
    let cash: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash'")
            .bind(luid)
            .fetch_one(&pool)
            .await
            .unwrap();
    let sales: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales'")
            .bind(luid)
            .fetch_one(&pool)
            .await
            .unwrap();

    let idem_key = "fixed-test-key-12345";
    let body = json!({
        "date": "2026-08-15",
        "description": "Idempotency test",
        "lines": [
            { "account_id": cash.to_string(), "direction": "DEBIT", "amount": "25.00" },
            { "account_id": sales.to_string(), "direction": "CREDIT", "amount": "25.00" }
        ]
    });

    let r1 = server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Idempotency-Key", idem_key)
        .json(&body)
        .send()
        .await
        .expect("first POST");
    assert_eq!(r1.status(), 201);
    let b1: serde_json::Value = r1.json().await.unwrap();

    let r2 = server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Idempotency-Key", idem_key)
        .json(&body)
        .send()
        .await
        .expect("replay POST");
    assert_eq!(r2.status(), 201, "replay must return 201");
    let b2: serde_json::Value = r2.json().await.unwrap();
    assert_eq!(
        b1["id"], b2["id"],
        "replay must return the same transaction id"
    );

    // Verify only one row was created.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(luid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "replay must NOT create a second transaction");
}

#[tokio::test]
async fn http_api_reports_trial_balance_200() {
    let server = TestServer::new().await;
    let (token, _uid) = bootstrap_with_token(&server, "api-tb@example.com").await;

    // Create a ledger (default chart of accounts is seeded by the
    // HTTP endpoint, not raw SQL — go through the API).
    let resp = server
        .client()
        .post(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({ "name": "TB Co", "base_currency": "USD" }))
        .send()
        .await
        .unwrap();
    let ledger: serde_json::Value = resp.json().await.unwrap();
    let ledger_id = ledger["id"].as_str().unwrap();

    let resp = server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{ledger_id}/reports/trial-balance",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .expect("GET trial balance");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["data"].is_array(),
        "trial balance response must include `data` array; got {body}"
    );
}

#[tokio::test]
async fn http_api_revoke_token_revokes() {
    let server = TestServer::new().await;
    let (token, user_id) = bootstrap_with_token(&server, "api-revoke@example.com").await;

    // Token works initially.
    let resp = server
        .client()
        .get(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "fresh token must work");

    // Revoke the token directly in the DB (mirrors what the
    // /account/api-tokens/revoke handler does).
    let pool = server.db().pool();
    sqlx::query("UPDATE api_tokens SET revoked_at = now() WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let resp = server
        .client()
        .get(format!("{}/api/v1/ledgers", server.base_url()))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        401,
        "revoked token must return 401; got {}",
        resp.status()
    );
}
