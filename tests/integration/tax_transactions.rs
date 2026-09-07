//! HTTP integration tests for tax on transactions (`a14-tax-on-transactions`).
//!
//! Covers:
//! - A taxed line posts a tax leg to the rate's account + a
//!   `posting_taxes` linkage (base + tax).
//! - The entry still has to balance including the tax leg.
//! - Unknown / inactive tax rates are rejected.
//! - A transaction without a tax still records nothing.
//! - Drafts keep their tax attribution through promotion.
//! - Reversing a taxed transaction negates the tax leg.
//! - The tax report aggregates net / tax / gross per rate.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use rust_decimal::Decimal;
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
/// ledger id, and EXPENSE / ASSET / LIABILITY account ids.
async fn setup(server: &TestServer, tag: &str) -> (reqwest::Client, Uuid, Uuid, Uuid, Uuid) {
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
    let (expense,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'EXPENSE' LIMIT 1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (asset,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' LIMIT 1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (liability,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'LIABILITY' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (client, ledger_id, expense, asset, liability)
}

/// Create a tax rate via the taxes form and return its id.
async fn create_tax_rate(
    client: &reqwest::Client,
    base: &str,
    ledger_id: Uuid,
    name: &str,
    rate: &str,
    kind: &str,
    account_id: Uuid,
    pool: &sqlx::PgPool,
) -> Uuid {
    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/taxes/new"))
        .form(&[
            ("name", name),
            ("rate", rate),
            ("kind", kind),
            ("account_id", &account_id.to_string()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "tax rate create should redirect");
    sqlx::query_scalar("SELECT id FROM tax_rates WHERE ledger_id = $1 AND name = $2")
        .bind(ledger_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Create a posted transaction via the create form. `tax` is the
/// tax rate id for line 0 ("" = none). Returns the txn id.
async fn create_txn(
    client: &reqwest::Client,
    base: &str,
    ledger_id: Uuid,
    expense: Uuid,
    asset: Uuid,
    tax: &str,
    credit_amount: &str,
) -> reqwest::Response {
    client
        .post(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "Office supplies with VAT"),
            ("action", "save"),
            ("lines[0][account_id]", &expense.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100.00"),
            ("lines[0][tax_rate_id]", tax),
            ("lines[1][account_id]", &asset.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", credit_amount),
        ])
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn taxed_transaction_posts_tax_leg_and_linkage() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, liability) = setup(&server, "taxok").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let rate_id = create_tax_rate(
        &client,
        &base,
        ledger_id,
        "VAT 10%",
        "0.1",
        "sales_tax",
        liability,
        &pool,
    )
    .await;

    let resp = create_txn(
        &client,
        &base,
        ledger_id,
        expense,
        asset,
        &rate_id.to_string(),
        "110.00",
    )
    .await;
    assert_eq!(resp.status(), 303);
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let txn_id: Uuid = loc.rsplit('/').next().unwrap().parse().unwrap();

    // Three postings: expense (100 debit), tax leg (10 debit), bank (110 credit).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM postings WHERE transaction_id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3, "expense + tax leg + bank");

    // The tax leg is on the rate's account, same side (debit), amount 10.
    let tax_leg: (Decimal, String, Option<String>) = sqlx::query_as(
        "SELECT amount, direction, memo FROM postings
         WHERE transaction_id = $1 AND account_id = $2",
    )
    .bind(txn_id)
    .bind(liability)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tax_leg.0, Decimal::new(10, 0), "tax = 100 * 0.1");
    assert_eq!(tax_leg.1, "DEBIT");
    assert!(tax_leg.2.unwrap_or_default().contains("VAT"));

    // Linkage recorded with base + tax.
    let link: (Uuid, Decimal, Decimal) = sqlx::query_as(
        "SELECT pt.tax_rate_id, pt.base_amount, pt.tax_amount
         FROM posting_taxes pt
         JOIN postings p ON p.id = pt.posting_id
         WHERE p.transaction_id = $1",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link.0, rate_id);
    assert_eq!(link.1, Decimal::new(100, 0));
    assert_eq!(link.2, Decimal::new(10, 0));
}

#[tokio::test]
async fn taxed_transaction_unbalanced_rejected() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, liability) = setup(&server, "taxbal").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let rate_id = create_tax_rate(
        &client,
        &base,
        ledger_id,
        "VAT 10%",
        "0.1",
        "sales_tax",
        liability,
        &pool,
    )
    .await;

    // Bank side only 100 — the tax leg of 10 unbalances the entry.
    let resp = create_txn(
        &client,
        &base,
        ledger_id,
        expense,
        asset,
        &rate_id.to_string(),
        "100.00",
    )
    .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("do not balance"),
        "expected a balance error, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn unknown_tax_rate_rejected() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, _) = setup(&server, "taxbad").await;
    let base = server.base_url();
    let bogus = Uuid::new_v4();
    let resp = create_txn(
        &client,
        &base,
        ledger_id,
        expense,
        asset,
        &bogus.to_string(),
        "110.00",
    )
    .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("tax rate"),
        "expected a tax-rate error, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn inactive_tax_rate_rejected() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, liability) = setup(&server, "taxoff").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let rate_id = create_tax_rate(
        &client,
        &base,
        ledger_id,
        "VAT 10%",
        "0.1",
        "sales_tax",
        liability,
        &pool,
    )
    .await;
    client
        .post(format!("{base}/ledgers/{ledger_id}/taxes/{rate_id}/toggle"))
        .send()
        .await
        .unwrap();
    let resp = create_txn(
        &client,
        &base,
        ledger_id,
        expense,
        asset,
        &rate_id.to_string(),
        "110.00",
    )
    .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("tax rate"),
        "expected an inactive-rate error, got: {}",
        body.chars().take(200).collect::<String>()
    );
}

#[tokio::test]
async fn no_tax_selected_records_nothing() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, _) = setup(&server, "taxnone").await;
    let base = server.base_url();
    let resp = create_txn(&client, &base, ledger_id, expense, asset, "", "100.00").await;
    assert_eq!(resp.status(), 303);
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let txn_id: Uuid = loc.rsplit('/').next().unwrap().parse().unwrap();

    let pool = server.db().pool();
    let postings: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM postings WHERE transaction_id = $1")
            .bind(txn_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(postings.0, 2);
    let links: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM posting_taxes pt
         JOIN postings p ON p.id = pt.posting_id WHERE p.transaction_id = $1",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(links.0, 0);
}

#[tokio::test]
async fn draft_promote_keeps_tax() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, liability) = setup(&server, "taxdraft").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let rate_id = create_tax_rate(
        &client,
        &base,
        ledger_id,
        "VAT 10%",
        "0.1",
        "sales_tax",
        liability,
        &pool,
    )
    .await;

    // Save as draft with tax.
    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "Draft with VAT"),
            ("action", "draft"),
            ("lines[0][account_id]", &expense.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100.00"),
            ("lines[0][tax_rate_id]", &rate_id.to_string()),
            ("lines[1][account_id]", &asset.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "110.00"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let draft_id: Uuid =
        sqlx::query_scalar("SELECT id FROM transactions WHERE ledger_id = $1 AND kind = 'draft'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Promote.
    client
        .post(format!(
            "{base}/ledgers/{ledger_id}/transactions/{draft_id}/post"
        ))
        .form(&[("_unused", "")])
        .send()
        .await
        .unwrap();

    let kind: (String,) = sqlx::query_as("SELECT kind FROM transactions WHERE id = $1")
        .bind(draft_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind.0, "standard");
    let link: (Decimal, Decimal) = sqlx::query_as(
        "SELECT pt.base_amount, pt.tax_amount FROM posting_taxes pt
         JOIN postings p ON p.id = pt.posting_id WHERE p.transaction_id = $1",
    )
    .bind(draft_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link.0, Decimal::new(100, 0));
    assert_eq!(link.1, Decimal::new(10, 0));
}

#[tokio::test]
async fn reversal_negates_tax_leg() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, liability) = setup(&server, "taxrev").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let rate_id = create_tax_rate(
        &client,
        &base,
        ledger_id,
        "VAT 10%",
        "0.1",
        "sales_tax",
        liability,
        &pool,
    )
    .await;

    let resp = create_txn(
        &client,
        &base,
        ledger_id,
        expense,
        asset,
        &rate_id.to_string(),
        "110.00",
    )
    .await;
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let txn_id: Uuid = loc.rsplit('/').next().unwrap().parse().unwrap();

    // Reverse.
    client
        .post(format!(
            "{base}/ledgers/{ledger_id}/transactions/{txn_id}/reverse"
        ))
        .form(&[("memo", "reversal test")])
        .send()
        .await
        .unwrap();

    let rev_id: Uuid = sqlx::query_scalar("SELECT id FROM transactions WHERE reverses_id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let tax_leg: (Decimal, String) = sqlx::query_as(
        "SELECT amount, direction FROM postings
         WHERE transaction_id = $1 AND account_id = $2",
    )
    .bind(rev_id)
    .bind(liability)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tax_leg.0, Decimal::new(10, 0));
    assert_eq!(tax_leg.1, "CREDIT", "reversal negates the tax leg");
}

#[tokio::test]
async fn tax_report_aggregates_base_tax_gross() {
    let server = TestServer::new().await;
    let (client, ledger_id, expense, asset, liability) = setup(&server, "taxrep").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let rate_id = create_tax_rate(
        &client,
        &base,
        ledger_id,
        "VAT 10%",
        "0.1",
        "sales_tax",
        liability,
        &pool,
    )
    .await;

    // Two taxed purchases: 100 + 10 and 50 + 5.
    create_txn(
        &client,
        &base,
        ledger_id,
        expense,
        asset,
        &rate_id.to_string(),
        "110.00",
    )
    .await;
    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .form(&[
            ("date", "2026-08-21"),
            ("description", "Second VAT purchase"),
            ("action", "save"),
            ("lines[0][account_id]", &expense.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "50.00"),
            ("lines[0][tax_rate_id]", &rate_id.to_string()),
            ("lines[1][account_id]", &asset.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "55.00"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let body = client
        .get(format!(
            "{base}/ledgers/{ledger_id}/taxes/report?from=2026-08-01&to=2026-08-31"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("VAT 10%"), "report should list the rate");
    assert!(body.contains("150.00"), "net base should be 100 + 50");
    assert!(body.contains("15.00"), "tax should be 10 + 5");
    assert!(body.contains("165.00"), "gross should be 150 + 15");
}
