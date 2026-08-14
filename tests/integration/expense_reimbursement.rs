//! Integration tests for the expense reimbursement module.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Reimb Co"),
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

async fn create_claim(
    client: &reqwest::Client,
    base_url: &str,
    cookie: &str,
    ledger_id: Uuid,
    title: &str,
) -> Uuid {
    let resp = post_form(
        client,
        &format!("{base_url}/ledgers/{ledger_id}/reimbursements"),
        cookie,
        &[
            ("title", title),
            ("employee_name", "Alice"),
            ("currency", "USD"),
        ],
    )
    .await;
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location")
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

#[tokio::test]
async fn http_create_draft_claim() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ),
        &cookie,
        &[
            ("title", "Trip to NYC"),
            ("employee_name", "Alice"),
            ("currency", "USD"),
        ],
    )
    .await;
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "create should redirect; got {status}"
    );
    // The list page should show the new claim.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET list");
    let body = resp.text().await.expect("list body");
    assert!(body.contains("Trip to NYC"));
}

#[tokio::test]
async fn http_create_claim_currency_mismatch() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements",
            server.base_url()
        ),
        &cookie,
        &[
            ("title", "Trip"),
            ("employee_name", "Alice"),
            ("currency", "EUR"),
        ],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 400, "currency mismatch should 400; body={body}");
    assert!(body.contains("currency mismatch"));
}


#[tokio::test]
async fn http_submit_empty_claim() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &cookie,
        ledger_id,
        "Empty",
    )
    .await;
    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 400);
    assert!(body.contains("empty claim"));
}

#[tokio::test]
async fn http_reject_without_reason() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("dave", "dave@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &cookie,
        ledger_id,
        "Trip",
    )
    .await;
    // Add a line.
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines",
            server.base_url()
        ),
        &cookie,
        &[
            ("txn_date", "2026-08-14"),
            ("description", "Coffee"),
            ("amount", "12.50"),
            ("gl_account_id", &other.to_string()),
        ],
    )
    .await;
    // Submit.
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    // Reject without reason.
    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/reject",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    // axum's form parser returns 422 for missing required
    // fields; our own check returns 400. Either way the body
    // should mention `reason`.
    assert!(status == 400 || status == 422);
    assert!(
        body.contains("reason"),
        "body should mention 'reason'; got: {body}"
    );
}

#[tokio::test]
async fn http_approve_idempotent_returns_409() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("eve", "eve@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;
    let _ = account_id(&pool, ledger_id, "Employee Payable").await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &cookie,
        ledger_id,
        "Trip",
    )
    .await;
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines",
            server.base_url()
        ),
        &cookie,
        &[
            ("txn_date", "2026-08-14"),
            ("description", "Coffee"),
            ("amount", "12.50"),
            ("gl_account_id", &other.to_string()),
        ],
    )
    .await;
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    // First approve.
    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/approve",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    assert_eq!(resp.status(), 303);
    // Second approve should return 409.
    let resp = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/approve",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn http_approve_creates_postings() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("frank", "frank@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(&pool, ledger_id, "Other Expense").await;
    let _ = account_id(&pool, ledger_id, "Employee Payable").await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &cookie,
        ledger_id,
        "Trip",
    )
    .await;
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines",
            server.base_url()
        ),
        &cookie,
        &[
            ("txn_date", "2026-08-14"),
            ("description", "Coffee"),
            ("amount", "12.50"),
            ("gl_account_id", &other.to_string()),
        ],
    )
    .await;
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    let _ = post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/approve",
            server.base_url()
        ),
        &cookie,
        &[],
    )
    .await;
    // 2 postings: DR Other Expense 12.50, CR Employee Payable 12.50.
    let (dr, cr): (Decimal, Decimal) = sqlx::query_as(
        r#"SELECT
             COALESCE(SUM(CASE WHEN direction='DEBIT'  THEN amount ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN direction='CREDIT' THEN amount ELSE 0 END), 0)
           FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND t.reference = (
             SELECT short_id FROM reimbursement_claims WHERE id = $2
           )"#,
    )
    .bind(ledger_id)
    .bind(claim_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dr, Decimal::new(1250, 2));
    assert_eq!(cr, Decimal::new(1250, 2));
}
