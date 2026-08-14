//! Integration tests for the WeChat Pay dedicated importer.
//! Drives the real axum router via `TestServer` with multipart
//! upload + urlencoded commit, and verifies the transactions
//! created.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

const WECHAT_5: &str = "\
微信支付账单明细,,,,,,,,,,
微信昵称：小明,,,,,,,,,,
起始时间：[2026-08-01 00:00:00] 终止时间：[2026-08-31 23:59:59],,,,,,,,,,,
导出类型：[全部],,,,,,,,,,,
共5笔记录,,,,,,,,,,
,,,,,,,,,,
交易时间,交易类型,交易对方,商品,收/支,金额(元),支付方式,当前状态,交易单号,商户单号,备注
2026-08-01 09:00:00,商户消费,商家A,商品A,支出,10.00,零钱,支付成功,1,,
2026-08-01 10:00:00,商户消费,商家B,商品B,支出,20.00,零钱,支付成功,2,,
2026-08-02 11:00:00,转账收款,老板C,转账,收入,30.00,零钱,收款成功,3,,
2026-08-02 12:00:00,商户消费,商家D,商品D,支出,40.00,零钱,支付成功,4,,
2026-08-03 13:00:00,转账收款,朋友E,转账,收入,50.00,零钱,收款成功,5,,
";

/// Minimal URL-encoder for form bodies.
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

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "WeChat Co"),
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
    let (id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2")
            .bind(ledger_id)
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("account exists");
    id
}

#[tokio::test]
async fn http_wechat_upload_previews_rows() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("wei", "wei@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let part = reqwest::multipart::Part::text(WECHAT_5.to_string()).file_name("wechat.csv");
    let form = reqwest::multipart::Form::new()
        .text("filename", "wechat.csv")
        .part("file", part);
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/import/wechat",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("upload");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("商家A"), "should contain payee 商家A");
    assert!(body.contains("wechat"), "should label format wechat");
    assert!(body.contains("5"), "should show 5 rows");
}

#[tokio::test]
async fn http_wechat_commit_creates_transactions() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("wei2", "wei2@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let expense = account_id(&pool, ledger_id, "Other Expense").await;

    let rows = openaccounting::import::wechat::parse(WECHAT_5.as_bytes(), &Default::default())
        .expect("parse")
        .0;
    assert_eq!(rows.len(), 5);
    let rows_json = serde_json::to_string(&rows).unwrap();
    let body = [
        ("filename".to_string(), "wechat.csv".to_string()),
        ("default_account_id".to_string(), cash.to_string()),
        ("expense_account_id".to_string(), expense.to_string()),
        ("rows".to_string(), rows_json),
    ]
    .iter()
    .map(|(k, v)| format!("{}={}", k, urlencode(v)))
    .collect::<Vec<_>>()
    .join("&");
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/import/wechat/commit",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("commit");
    let status = resp.status();
    let resp_body = resp.text().await.expect("commit body");
    assert!(
        status == 303 || status == 302,
        "commit should redirect; got {status} body={resp_body}"
    );

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 5);
    let (sum_d, sum_c): (Decimal, Decimal) = sqlx::query_as(
        r#"SELECT
             COALESCE(SUM(CASE WHEN direction='DEBIT'  THEN amount ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN direction='CREDIT' THEN amount ELSE 0 END), 0)
           FROM postings p JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1"#,
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sum_d, sum_c, "trial balance must balance");
    assert_eq!(sum_d, Decimal::new(15000, 2));
}

#[tokio::test]
async fn http_wechat_commit_rolls_back_on_bad_row() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("wei3", "wei3@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let expense = account_id(&pool, ledger_id, "Other Expense").await;

    let mut rows = openaccounting::import::wechat::parse(WECHAT_5.as_bytes(), &Default::default())
        .expect("parse")
        .0;
    // Corrupt row 3 with a zero amount.
    rows[2].credit = "0.00".to_string();
    let rows_json = serde_json::to_string(&rows).unwrap();
    let body = [
        ("filename".to_string(), "wechat.csv".to_string()),
        ("default_account_id".to_string(), cash.to_string()),
        ("expense_account_id".to_string(), expense.to_string()),
        ("rows".to_string(), rows_json),
    ]
    .iter()
    .map(|(k, v)| format!("{}={}", k, urlencode(v)))
    .collect::<Vec<_>>()
    .join("&");
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/import/wechat/commit",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("commit");
    let status = resp.status();
    let resp_body = resp.text().await.expect("commit body");
    assert_eq!(status, 422, "bad row should 422; body={resp_body}");
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "no rows should be committed on rollback");
}
