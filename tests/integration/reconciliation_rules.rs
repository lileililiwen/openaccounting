//! Integration tests for the reconciliation rules engine.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Rules Co"),
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

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric()
            || matches!(c, '-' | '.' | '_' | '~' | '{' | '}' | '"' | ':' | ',')
        {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

async fn post(
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
async fn http_rules_list_empty() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = server
        .client()
        .get(format!("{}/ledgers/{}/rules", server.base_url(), ledger_id))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET");
    let body = resp.text().await.expect("body");
    assert!(body.contains("Reconciliation Rules"));
    assert!(body.contains("No rules yet"));
}

#[tokio::test]
async fn http_rule_create_categorize_persists() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let gl = account_id(&pool, ledger_id, "Other Expense").await;

    let resp = post(
        server.client(),
        &format!("{}/ledgers/{}/rules", server.base_url(), ledger_id),
        &cookie,
        &[
            ("name", "Starbucks = Other Expense"),
            ("kind", "categorize"),
            ("priority", "10"),
            ("predicate", r#"{"payee_glob":"STARBUCKS%"}"#),
            ("action", &format!(r#"{{"gl_account_id":"{}"}}"#, gl)),
        ],
    )
    .await;
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "expected redirect, got {status}"
    );
    // Verify the row is in the DB.
    let (name, kind, priority, is_active): (String, String, i32, bool) = sqlx::query_as(
        "SELECT name, kind, priority, is_active FROM reconciliation_rules WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(name, "Starbucks = Other Expense");
    assert_eq!(kind, "categorize");
    assert_eq!(priority, 10);
    assert!(is_active);
}

#[tokio::test]
async fn http_rule_create_rejects_invalid_predicate_json() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let _ = account_id(&pool, ledger_id, "Other Expense").await;
    let resp = post(
        server.client(),
        &format!("{}/ledgers/{}/rules", server.base_url(), ledger_id),
        &cookie,
        &[
            ("name", "Bad"),
            ("kind", "categorize"),
            ("priority", "10"),
            ("predicate", "not json"),
            ("action", "{}"),
        ],
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 400, "bad JSON should return 400; body={body}");
    assert!(body.contains("not valid JSON"));
}

#[tokio::test]
async fn http_rule_toggle_flips_is_active() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("dave", "dave@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    // Insert directly to skip the form.
    let (rule_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO reconciliation_rules
              (ledger_id, name, kind, priority, predicate, action, is_active)
           VALUES ($1, 'r', 'categorize', 10, '{}'::jsonb, '{}'::jsonb, TRUE)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let resp = post(
        server.client(),
        &format!(
            "{}/ledgers/{}/rules/{}/toggle",
            server.base_url(),
            ledger_id,
            rule_id
        ),
        &cookie,
        &[],
    )
    .await;
    assert_eq!(resp.status(), 303);
    let (active,): (bool,) =
        sqlx::query_as("SELECT is_active FROM reconciliation_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!active);
    // Toggle again — should flip back.
    let _ = post(
        server.client(),
        &format!(
            "{}/ledgers/{}/rules/{}/toggle",
            server.base_url(),
            ledger_id,
            rule_id
        ),
        &cookie,
        &[],
    )
    .await;
    let (active,): (bool,) =
        sqlx::query_as("SELECT is_active FROM reconciliation_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(active);
}

#[tokio::test]
async fn http_rule_priority_tiebreak_picks_lower() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("eve", "eve@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let gl = account_id(&pool, ledger_id, "Other Expense").await;

    let r1 = create_rule(
        &pool,
        ledger_id,
        "high",
        200,
        r#"{"payee_glob":"STARBUCKS%"}"#,
        &format!(r#"{{"gl_account_id":"{}"}}"#, gl),
    )
    .await;
    let _r2 = create_rule(
        &pool,
        ledger_id,
        "low",
        10,
        r#"{"payee_glob":"STARBUCKS%"}"#,
        &format!(r#"{{"gl_account_id":"{}"}}"#, gl),
    )
    .await;

    // The list page shows rules ordered by priority.
    let resp = server
        .client()
        .get(format!("{}/ledgers/{}/rules", server.base_url(), ledger_id))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET");
    let body = resp.text().await.expect("body");
    let low_idx = body.find("low").expect("low in body");
    let high_idx = body.find("high").expect("high in body");
    assert!(
        low_idx < high_idx,
        "low-priority rule should appear before high-priority"
    );
    let _ = r1;
}

async fn create_rule(
    pool: &PgPool,
    ledger_id: Uuid,
    name: &str,
    priority: i32,
    predicate: &str,
    action: &str,
) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO reconciliation_rules
              (ledger_id, name, kind, priority, predicate, action, is_active)
           VALUES ($1, $2, 'categorize', $3, $4::jsonb, $5::jsonb, TRUE)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(priority)
    .bind(predicate)
    .bind(action)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn http_rule_create_rejects_unknown_kind() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("frank", "frank@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let _ = account_id(&pool, ledger_id, "Other Expense").await;
    let resp = post(
        server.client(),
        &format!("{}/ledgers/{}/rules", server.base_url(), ledger_id),
        &cookie,
        &[
            ("name", "Bad"),
            ("kind", "weird_kind"),
            ("priority", "10"),
            ("predicate", "{}"),
            ("action", "{}"),
        ],
    )
    .await;
    assert_eq!(resp.status(), 400);
    let body = resp.text().await.expect("body");
    assert!(body.contains("unknown kind"));
}
