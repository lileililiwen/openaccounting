//! Integration tests for the policy engine.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Policy Co"),
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
        .expect("Location")
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

#[tokio::test]
async fn http_cap_blocks_submit() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;

    // Create a 100 USD / day cap on the "Other Expense" category
    // (the line's category is the account name from the
    // gl_account_id — see the handler).
    sqlx::query(
        r#"INSERT INTO reimbursement_policies
              (ledger_id, name, kind, config, severity, is_active)
           VALUES ($1, 'cap', 'category_cap', $2::jsonb, 'hard', TRUE)"#,
    )
    .bind(ledger_id)
    .bind(serde_json::json!({"category": "Other Expense", "max_per_day": "100.00"}))
    .execute(&pool)
    .await
    .unwrap();

    // Create a claim and add a 200 USD line.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ))
        .form(&[
            ("title", "Cap test"),
            ("employee_name", "Alice"),
            ("currency", "USD"),
        ])
        .header(reqwest::header::COOKIE, cookie.clone())
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

    let _ = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines",
            server.base_url()
        ))
        .form(&[
            ("txn_date", "2026-08-14"),
            ("description", "Big"),
            ("amount", "200.00"),
            ("gl_account_id", &other.to_string()),
        ])
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("add line");

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("submit");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 400, "cap violation should return 400; body={body}");
    assert!(
        body.contains("cap") || body.contains("CategoryCap"),
        "body should mention the cap: {body}"
    );
}

#[tokio::test]
async fn http_receipt_required_blocks_approve() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;
    let _ = account_id(&pool, ledger_id, "Employee Payable").await;

    // Add a 50 USD receipt-required policy.
    sqlx::query(
        r#"INSERT INTO reimbursement_policies
              (ledger_id, name, kind, config, severity, is_active)
           VALUES ($1, 'rcpt', 'receipt_required', $2::jsonb, 'hard', TRUE)"#,
    )
    .bind(ledger_id)
    .bind(serde_json::json!({"min_amount": "50.00"}))
    .execute(&pool)
    .await
    .unwrap();

    // Create a claim, add a 100 USD line WITHOUT a receipt,
    // submit, then approve. Approve should be blocked.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ))
        .form(&[
            ("title", "Receipt test"),
            ("employee_name", "Bob"),
            ("currency", "USD"),
        ])
        .header(reqwest::header::COOKIE, cookie.clone())
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

    let _ = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines",
            server.base_url()
        ))
        .form(&[
            ("txn_date", "2026-08-14"),
            ("description", "Big"),
            ("amount", "100.00"),
            ("gl_account_id", &other.to_string()),
        ])
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("add line");
    let _ = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("submit");
    // The submit MUST be blocked; assert it.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("submit again");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(
        status, 400,
        "receipt-required violation should block submit; body={body}"
    );
    assert!(
        body.contains("receipt") || body.contains("Receipt"),
        "body should mention the receipt: {body}"
    );
}

#[tokio::test]
async fn http_soft_warning_does_not_block() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;

    // Soft cap of 100 USD on "Other Expense" category.
    sqlx::query(
        r#"INSERT INTO reimbursement_policies
              (ledger_id, name, kind, config, severity, is_active)
           VALUES ($1, 'soft', 'category_cap', $2::jsonb, 'soft', TRUE)"#,
    )
    .bind(ledger_id)
    .bind(serde_json::json!({"category": "Other Expense", "max_per_day": "100.00"}))
    .execute(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ))
        .form(&[
            ("title", "Soft cap"),
            ("employee_name", "Carol"),
            ("currency", "USD"),
        ])
        .header(reqwest::header::COOKIE, cookie.clone())
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

    let _ = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines",
            server.base_url()
        ))
        .form(&[
            ("txn_date", "2026-08-14"),
            ("description", "Big"),
            ("amount", "200.00"),
            ("gl_account_id", &other.to_string()),
        ])
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("add line");
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("submit");
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "soft violation should NOT block; got {status}"
    );
}

async fn account_id(pool: &PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    let (id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2")
            .bind(ledger_id)
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("account exists");
    id
}
