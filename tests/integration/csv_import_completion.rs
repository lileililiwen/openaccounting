//! Integration tests for the generic CSV importer. These
//! tests cover the post-completion contract: the `confirm`
//! handler actually inserts transactions, rolls back on bad
//! rows, and respects the `default_account_id` fallback.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

const CSV_HEADER: &str = "date,description,debit,credit,account,payee,reference\n";

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "CSV Co"),
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

async fn account_id(pool: &PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2",
    )
    .bind(ledger_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("account exists");
    id
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

async fn post_form(
    client: &reqwest::Client,
    url: &str,
    cookie: &str,
    fields: &[(&str, &str)],
) -> reqwest::Response {
    let body = fields
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    client
        .post(url)
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("post")
}

#[tokio::test]
async fn http_csv_import_creates_n_transactions() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;

    // Build a 12-row CSV (all valid).
    let mut csv = String::from(CSV_HEADER);
    for i in 1..=12 {
        csv.push_str(&format!(
            "2026-08-{:02},Item {},10.00,,,Supplier{},\n",
            i, i, i
        ));
    }

    // Re-parse to build the JSON rows the confirm handler wants.
    let rows = csv.as_str();
    let parsed = openaccounting::import::csv::parse(rows);
    let rows_json = serde_json::to_string(&parsed).unwrap();

    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{}/import/confirm",
            server.base_url(),
            ledger_id
        ),
        &cookie,
        &[
            ("filename", "items.csv"),
            ("date_column", "0"),
            ("description_column", "1"),
            ("debit_column", "2"),
            ("credit_column", "3"),
            ("account_id", &other.to_string()),
            ("default_account_id", &other.to_string()),
            ("skip_duplicates", "true"),
            ("rows", &rows_json),
        ],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("confirm body");
    assert!(
        status == 303 || status == 302,
        "should redirect; got {status} body={body}"
    );

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 12, "expected 12 transactions, got {count}");
}

#[tokio::test]
async fn http_csv_import_rolls_back_on_bad_date() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;

    // 1 bad + 9 good. The bad row is in position 1.
    let mut csv = String::from(CSV_HEADER);
    csv.push_str("2026-13-99,Bad,10.00,,,X,\n");
    for i in 1..=9 {
        csv.push_str(&format!(
            "2026-08-{:02},Item {},10.00,,,Supplier{},\n",
            i, i, i
        ));
    }
    let parsed = openaccounting::import::csv::parse(&csv);
    let rows_json = serde_json::to_string(&parsed).unwrap();

    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{}/import/confirm",
            server.base_url(),
            ledger_id
        ),
        &cookie,
        &[
            ("filename", "items.csv"),
            ("account_id", &other.to_string()),
            ("default_account_id", &other.to_string()),
            ("skip_duplicates", "false"),
            ("rows", &rows_json),
        ],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 422, "bad-date batch should return 422");
    assert!(body.contains("rolled back"), "body should explain: {body}");

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "expected 0 committed on rollback");
}

#[tokio::test]
async fn http_csv_import_rejects_both_debit_and_credit() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;

    let csv = format!(
        "{CSV_HEADER}2026-08-01,Bad,10.00,10.00,,X,\n"
    );
    let parsed = openaccounting::import::csv::parse(&csv);
    let rows_json = serde_json::to_string(&parsed).unwrap();

    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{}/import/confirm",
            server.base_url(),
            ledger_id
        ),
        &cookie,
        &[
            ("filename", "items.csv"),
            ("account_id", &other.to_string()),
            ("default_account_id", &other.to_string()),
            ("skip_duplicates", "false"),
            ("rows", &rows_json),
        ],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(
        status, 422,
        "both-debit-and-credit should return 422; body={body}"
    );
    assert!(
        body.contains("rolled back") || body.contains("debit"),
        "body should explain: {body}"
    );
}

#[tokio::test]
async fn http_csv_import_unresolved_account_falls_back() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("dave", "dave@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;

    // Row has an empty `account` column; the handler should use
    // `default_account_id` (the `Other Expense` account).
    let csv = format!(
        "{CSV_HEADER}2026-08-01,NoAccount,10.00,,,Payee,\n"
    );
    let parsed = openaccounting::import::csv::parse(&csv);
    let rows_json = serde_json::to_string(&parsed).unwrap();

    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{}/import/confirm",
            server.base_url(),
            ledger_id
        ),
        &cookie,
        &[
            ("filename", "items.csv"),
            ("account_id", &other.to_string()),
            ("default_account_id", &other.to_string()),
            ("skip_duplicates", "false"),
            ("rows", &rows_json),
        ],
    )
    .await;
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "should redirect; got {status}"
    );
    // Verify the posting went to Other Expense + Cash on Hand.
    let (accts,): (Vec<String>,) = sqlx::query_as(
        r#"SELECT array_agg(a.name ORDER BY a.name)
           FROM postings p
           JOIN accounts a ON a.id = p.account_id
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1"#,
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let names: Vec<String> = accts;
    assert!(names.contains(&"Other Expense".to_string()));
    assert!(names.contains(&"Cash on Hand".to_string()));
    let _ = cash;
}
