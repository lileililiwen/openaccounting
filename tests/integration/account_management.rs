//! HTTP integration tests for account management + opening balances
//! (`a15-account-management`).
//!
//! Covers:
//! - Edit name/code/description; audit logged.
//! - Type/subtype locked once an account has postings.
//! - Type/subtype editable without postings.
//! - Archive hides from pickers but keeps reports; activate restores.
//! - Opening balances post one balanced entry against Opening Balances.
//! - Second opening-balances save refused; all-zero refused.

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

/// Register + log in, create a ledger, return client + ledger id + a
/// freshly created account id.
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

    let resp = client
        .post(format!("{}/ledgers/{ledger_id}/accounts/new", server.base_url()))
        .form(&[
            ("name", "Probe Account"),
            ("code", "9990"),
            ("account_type", "EXPENSE"),
            ("account_subtype", "OPERATING_EXPENSE"),
            ("description", "test"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let pool = server.db().pool();
    let (account_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Probe Account'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    (client, ledger_id, account_id)
}

async fn bank_account(pool: &sqlx::PgPool, ledger_id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'")
        .bind(ledger_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Post a small transaction using `account` as a DEBIT leg so it has
/// postings.
async fn give_account_postings(
    client: &reqwest::Client,
    base: &str,
    ledger_id: Uuid,
    account: Uuid,
    bank: Uuid,
) {
    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .form(&[
            ("date", "2026-08-20"),
            ("description", "Give postings"),
            ("action", "save"),
            ("lines[0][account_id]", &account.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "5.00"),
            ("lines[1][account_id]", &bank.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "5.00"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
}

#[tokio::test]
async fn edit_account_updates_fields_and_audits() {
    let server = TestServer::new().await;
    let (client, ledger_id, account_id) = setup(&server, "accedit").await;
    let base = server.base_url();
    let pool = server.db().pool();

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/accounts/{account_id}/edit"))
        .form(&[
            ("name", "Renamed Account"),
            ("code", "9991"),
            ("description", "renamed"),
            ("account_type", "EXPENSE"),
            ("account_subtype", "OPERATING_EXPENSE"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let row: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT name, code, description FROM accounts WHERE id = $1",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Renamed Account");
    assert_eq!(row.1.as_deref(), Some("9991"));
    assert_eq!(row.2.as_deref(), Some("renamed"));

    let audit_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_entries WHERE entity_type = 'account' AND entity_id = $1 AND action = 'update'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_count.0 >= 1, "edit should be audit logged");
}

#[tokio::test]
async fn edit_type_locked_once_account_has_postings() {
    let server = TestServer::new().await;
    let (client, ledger_id, account_id) = setup(&server, "acclock").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let bank = bank_account(&pool, ledger_id).await;
    give_account_postings(&client, &base, ledger_id, account_id, bank).await;

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/accounts/{account_id}/edit"))
        .form(&[
            ("name", "Renamed Account"),
            ("code", "9991"),
            ("description", ""),
            ("account_type", "ASSET"),
            ("account_subtype", "CURRENT_ASSET"),
        ])
        .send()
        .await
        .unwrap();
    // The type change is ignored (kept at the existing classification)
    // and the edit still succeeds for the editable fields.
    assert_eq!(resp.status(), 303);

    // The account keeps its original classification.
    let (ty, st): (String, String) =
        sqlx::query_as("SELECT type, subtype FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ty, "EXPENSE");
    assert_eq!(st, "OPERATING_EXPENSE");

    // The GET edit page surfaces the lock notice.
    let edit_page = client
        .get(format!("{base}/ledgers/{ledger_id}/accounts/{account_id}/edit"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        edit_page.contains("locked") && edit_page.contains("already has postings"),
        "expected a lock notice on the edit page, got: {}",
        edit_page.chars().take(300).collect::<String>()
    );
}

#[tokio::test]
async fn edit_type_allowed_without_postings() {
    let server = TestServer::new().await;
    let (client, ledger_id, account_id) = setup(&server, "accmove").await;
    let base = server.base_url();
    let pool = server.db().pool();

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/accounts/{account_id}/edit"))
        .form(&[
            ("name", "Moved Account"),
            ("code", "9992"),
            ("description", ""),
            ("account_type", "ASSET"),
            ("account_subtype", "CURRENT_ASSET"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let (ty, st): (String, String) =
        sqlx::query_as("SELECT type, subtype FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ty, "ASSET");
    assert_eq!(st, "CURRENT_ASSET");
}

#[tokio::test]
async fn archive_hides_from_pickers_keeps_in_reports() {
    let server = TestServer::new().await;
    let (client, ledger_id, account_id) = setup(&server, "accarch").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let bank = bank_account(&pool, ledger_id).await;
    give_account_postings(&client, &base, ledger_id, account_id, bank).await;

    let resp = client
        .post(format!(
            "{base}/ledgers/{ledger_id}/accounts/{account_id}/toggle-archive"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let archived: bool = sqlx::query_scalar("SELECT is_archived FROM accounts WHERE id = $1")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(archived);

    // Not in the new-transaction picker.
    let picker = client
        .get(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        !picker.contains("Probe Account"),
        "archived account must be hidden from the picker"
    );

    // Still in the trial balance report.
    let report = client
        .get(format!("{base}/ledgers/{ledger_id}/reports/trial-balance"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        report.contains("Probe Account"),
        "archived account must stay in reports"
    );
}

#[tokio::test]
async fn activate_restores_account_in_picker() {
    let server = TestServer::new().await;
    let (client, ledger_id, account_id) = setup(&server, "accact").await;
    let base = server.base_url();

    client
        .post(format!(
            "{base}/ledgers/{ledger_id}/accounts/{account_id}/toggle-archive"
        ))
        .send()
        .await
        .unwrap();
    client
        .post(format!(
            "{base}/ledgers/{ledger_id}/accounts/{account_id}/toggle-archive"
        ))
        .send()
        .await
        .unwrap();

    let pool = server.db().pool();
    let archived: bool = sqlx::query_scalar("SELECT is_archived FROM accounts WHERE id = $1")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!archived);

    let picker = client
        .get(format!("{base}/ledgers/{ledger_id}/transactions/new"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(picker.contains("Probe Account"));
}

#[tokio::test]
async fn opening_balances_post_balanced_entry() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "obok").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let bank = bank_account(&pool, ledger_id).await;

    let amount = "amount[".to_string() + &bank.to_string() + "]";
    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/opening-balances"))
        .form(&[("date", "2026-01-01"), (amount.as_str(), "1000.00")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let (txn_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM transactions WHERE ledger_id = $1 AND description = 'Opening balances'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Balanced: bank debit 1000, contra credit 1000 on Opening Balances.
    let legs: Vec<(String, Decimal)> =
        sqlx::query_as("SELECT direction, amount FROM postings WHERE transaction_id = $1")
            .bind(txn_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let mut debits = Decimal::ZERO;
    let mut credits = Decimal::ZERO;
    for (d, a) in legs {
        if d == "DEBIT" {
            debits += a;
        } else {
            credits += a;
        }
    }
    assert_eq!(debits, Decimal::new(1000, 0));
    assert_eq!(credits, Decimal::new(1000, 0));

    let contra: (String,) = sqlx::query_as(
        "SELECT a.name FROM postings p JOIN accounts a ON a.id = p.account_id
         WHERE p.transaction_id = $1 AND a.name = 'Opening Balances'",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(contra.0, "Opening Balances");
}

#[tokio::test]
async fn opening_balances_second_entry_refused() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "obdup").await;
    let base = server.base_url();
    let pool = server.db().pool();
    let bank = bank_account(&pool, ledger_id).await;

    let amount = "amount[".to_string() + &bank.to_string() + "]";
    client
        .post(format!("{base}/ledgers/{ledger_id}/opening-balances"))
        .form(&[("date", "2026-01-01"), (amount.as_str(), "1000.00")])
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/opening-balances"))
        .form(&[("date", "2026-01-01"), (amount.as_str(), "500.00")])
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("already been recorded"),
        "expected a 'already recorded' notice, got: {}",
        body.chars().take(300).collect::<String>()
    );

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND description = 'Opening balances'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1, "only one opening-balances entry may exist");
}

#[tokio::test]
async fn opening_balances_all_zero_creates_nothing() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "obzero").await;
    let base = server.base_url();
    let pool = server.db().pool();

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/opening-balances"))
        .form(&[("date", "2026-01-01")])
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Enter at least one"),
        "expected a notice, got: {}",
        body.chars().take(300).collect::<String>()
    );

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND description = 'Opening balances'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count.0, 0);
}
