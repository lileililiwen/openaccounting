//! HTTP integration tests for the onboarding setup checklist
//! (`a16-onboarding-quickstart`).

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

/// Register + log in + create a ledger; return client, ledger id, owner id.
async fn setup(server: &TestServer, tag: &str) -> (reqwest::Client, Uuid, Uuid) {
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
    let (owner,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    (client, ledger_id, owner)
}

async fn setup_page(client: &reqwest::Client, base: &str, ledger_id: Uuid) -> String {
    client
        .get(format!("{base}/ledgers/{ledger_id}/setup"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

#[tokio::test]
async fn fresh_ledger_shows_five_pending_steps() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "obfresh").await;
    let body = setup_page(&client, &server.base_url(), ledger_id).await;
    for label in [
        "Set opening balances",
        "Record your first transaction",
        "Attach a receipt or invoice",
        "Link a bank account",
        "Invite a collaborator",
    ] {
        assert!(body.contains(label), "missing milestone: {label}");
    }
    assert!(
        body.contains("0 of 5 steps done"),
        "fresh ledger should show 0 of 5, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn opening_balances_marks_first_milestone() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "obopening").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let bank: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let amount = "amount[".to_string() + &bank.to_string() + "]";
    client
        .post(format!("{base}/ledgers/{ledger_id}/opening-balances"))
        .form(&[("date", "2026-01-01"), (amount.as_str(), "1000.00")])
        .send()
        .await
        .unwrap();

    let body = setup_page(&client, &base, ledger_id).await;
    assert!(
        body.contains("1 of 5 steps done"),
        "expected 1 of 5, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn first_transaction_marks_milestone() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "obtxn").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'EXPENSE' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let bank: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    client
        .post(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "First real expense"),
            ("action", "save"),
            ("lines[0][account_id]", &expense.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "10.00"),
            ("lines[1][account_id]", &bank.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "10.00"),
        ])
        .send()
        .await
        .unwrap();

    let body = setup_page(&client, &base, ledger_id).await;
    assert!(
        body.contains("1 of 5 steps done"),
        "expected 1 of 5, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn document_bank_feed_and_invitation_each_mark_milestones() {
    let server = TestServer::new().await;
    let (client, ledger_id, owner) = setup(&server, "obsteps").await;
    let base = server.base_url();
    let pool = server.db().pool();

    // Document.
    sqlx::query(
        "INSERT INTO documents (id, ledger_id, transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by)
         VALUES (gen_random_uuid(), $1, NULL, 't.txt', 't.txt', 'text/plain', 1, $2)",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();
    let body = setup_page(&client, &base, ledger_id).await;
    assert!(body.contains("1 of 5 steps done"), "document should count");

    // Bank feed link.
    sqlx::query(
        "INSERT INTO bank_feed_links (ledger_id, provider, status) VALUES ($1, 'manual', 'active')",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();
    let body = setup_page(&client, &base, ledger_id).await;
    assert!(body.contains("2 of 5 steps done"), "bank feed should count");

    // Invitation.
    sqlx::query(
        "INSERT INTO ledger_invitations (ledger_id, inviter_id, invitee_email, role) VALUES ($1, $2, 'accountant@example.com', 'viewer')",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();
    let body = setup_page(&client, &base, ledger_id).await;
    assert!(
        body.contains("3 of 5 steps done"),
        "invitation should count"
    );
}

#[tokio::test]
async fn dashboard_card_shows_then_hides() {
    let server = TestServer::new().await;
    let (client, ledger_id, owner) = setup(&server, "obdash").await;
    let base = server.base_url();
    let pool = server.db().pool();

    // Incomplete: card shows.
    let dash = client
        .get(format!("{base}/ledgers/{ledger_id}/dashboard"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        dash.contains("Get your books set up"),
        "setup card should show while incomplete"
    );

    // Complete everything: opening balances, first transaction, document,
    // bank feed, invitation.
    let bank: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'EXPENSE' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let amount = "amount[".to_string() + &bank.to_string() + "]";
    client
        .post(format!("{base}/ledgers/{ledger_id}/opening-balances"))
        .form(&[("date", "2026-01-01"), (amount.as_str(), "100.00")])
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "Post-opening expense"),
            ("action", "save"),
            ("lines[0][account_id]", &expense.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "5.00"),
            ("lines[1][account_id]", &bank.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "5.00"),
        ])
        .send()
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO documents (id, ledger_id, transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by)
         VALUES (gen_random_uuid(), $1, NULL, 'd.txt', 'd.txt', 'text/plain', 1, $2)",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO bank_feed_links (ledger_id, provider, status) VALUES ($1, 'manual', 'active')",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ledger_invitations (ledger_id, inviter_id, invitee_email, role) VALUES ($1, $2, 'acc2@example.com', 'viewer')",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();

    let dash = client
        .get(format!("{base}/ledgers/{ledger_id}/dashboard"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        !dash.contains("Get your books set up"),
        "setup card should hide once complete"
    );

    let body = setup_page(&client, &base, ledger_id).await;
    assert!(
        body.contains("All 5 steps are done"),
        "setup page should show all done, got: {}",
        body.chars().take(200).collect::<String>()
    );
}
