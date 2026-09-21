//! Integration tests for statement reconciliation sessions
//! (`statement-reconciliation`): open → clear → finish (zero-gate,
//! lock) → unreconcile-with-reason, plus CAMT/QBO import and rule
//! suggestions. Drives the real axum router via `TestServer`.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "RecSession Co"),
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
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

async fn post_form(
    server: &TestServer,
    cookie: &str,
    url: &str,
    fields: &[(&str, &str)],
) -> reqwest::Response {
    let body = fields
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    server
        .client()
        .post(url)
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("post form")
}

async fn insert_line(
    pool: &PgPool,
    ledger_id: Uuid,
    account_id: Uuid,
    date: &str,
    desc: &str,
    amount: Decimal,
) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO bank_statement_lines (ledger_id, account_id, statement_date, description, amount)
           VALUES ($1, $2, $3::date, $4, $5) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .bind(date)
    .bind(desc)
    .bind(amount)
    .fetch_one(pool)
    .await
    .expect("insert line");
    id
}

async fn create_session(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    account_id: Uuid,
    close_date: &str,
    close_balance: &str,
) -> (reqwest::StatusCode, Option<Uuid>) {
    let resp = post_form(
        server,
        cookie,
        &format!(
            "{}/ledgers/{}/reconcile/{}/sessions",
            server.base_url(),
            ledger_id,
            account_id
        ),
        &[
            ("stmt_close_date", close_date),
            ("stmt_close_balance", close_balance),
        ],
    )
    .await;
    let status = resp.status();
    let id = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|loc| loc.rsplit('/').next().unwrap().parse().ok());
    (status, id)
}

async fn session_status(pool: &PgPool, id: Uuid) -> String {
    let (s,): (String,) = sqlx::query_as("SELECT status FROM rec_sessions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("session exists");
    s
}

// ─── 1.4 / 1.6: nonzero difference blocks finish ─────────────────────────────

#[tokio::test]
async fn finish_with_nonzero_difference_leaves_session_open() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec1", "rec1@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;

    let l1 = insert_line(
        &pool,
        ledger_id,
        cash,
        "2026-08-21",
        "ACME",
        Decimal::new(10000, 2),
    )
    .await;
    let (_status, sid) =
        create_session(&server, &cookie, ledger_id, cash, "2026-08-31", "125.10").await;
    let sid = sid.expect("session created");

    // Clear the only line: opening 0 + cleared 100 vs close 125.10.
    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reconcile/{}/sessions/{}/clear",
            server.base_url(),
            ledger_id,
            cash,
            sid
        ),
        &[("bank_line_id", &l1.to_string()), ("confirm", "1")],
    )
    .await;
    assert!(resp.status().is_redirection(), "clear redirects");

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reconcile/{}/sessions/{}/finish",
            server.base_url(),
            ledger_id,
            cash,
            sid
        ),
        &[],
    )
    .await;
    assert_eq!(resp.status(), 409, "nonzero finish is a conflict");
    let body = resp.text().await.expect("body");
    assert!(
        body.contains("25.10"),
        "409 body shows the difference: {body}"
    );
    assert_eq!(session_status(&pool, sid).await, "open");
}

// ─── 1.4: zero difference closes and locks ───────────────────────────────────

#[tokio::test]
async fn finish_at_zero_closes_and_locks() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec2", "rec2@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;

    let l1 = insert_line(
        &pool,
        ledger_id,
        cash,
        "2026-08-21",
        "ACME",
        Decimal::new(10000, 2),
    )
    .await;
    let (_status, sid) =
        create_session(&server, &cookie, ledger_id, cash, "2026-08-31", "100.00").await;
    let sid = sid.expect("session created");

    let clear_url = format!(
        "{}/ledgers/{}/reconcile/{}/sessions/{}/clear",
        server.base_url(),
        ledger_id,
        cash,
        sid
    );
    let resp = post_form(
        &server,
        &cookie,
        &clear_url,
        &[("bank_line_id", &l1.to_string())],
    )
    .await;
    assert_eq!(resp.status(), 400, "clear without confirm is rejected");

    let resp = post_form(
        &server,
        &cookie,
        &clear_url,
        &[("bank_line_id", &l1.to_string()), ("confirm", "1")],
    )
    .await;
    assert!(resp.status().is_redirection());

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reconcile/{}/sessions/{}/finish",
            server.base_url(),
            ledger_id,
            cash,
            sid
        ),
        &[],
    )
    .await;
    assert!(resp.status().is_redirection(), "zero finish redirects");
    assert_eq!(session_status(&pool, sid).await, "closed");

    // Lock: unclear on a closed session fails with 409.
    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reconcile/{}/sessions/{}/unclear",
            server.base_url(),
            ledger_id,
            cash,
            sid
        ),
        &[("bank_line_id", &l1.to_string())],
    )
    .await;
    assert_eq!(resp.status(), 409, "locked session rejects unclear");
    let body = resp.text().await.expect("body");
    assert!(
        body.contains("locked"),
        "lock message names the lock: {body}"
    );
}

// ─── 1.7: unreconcile gates ──────────────────────────────────────────────────

async fn closed_session(
    server: &TestServer,
    pool: &PgPool,
    cookie: &str,
    ledger_id: Uuid,
    cash: Uuid,
) -> Uuid {
    let l1 = insert_line(
        pool,
        ledger_id,
        cash,
        "2026-08-21",
        "ACME",
        Decimal::new(5000, 2),
    )
    .await;
    let (_s, sid) = create_session(server, cookie, ledger_id, cash, "2026-08-31", "50.00").await;
    let sid = sid.expect("session created");
    let base = format!(
        "{}/ledgers/{}/reconcile/{}/sessions/{}",
        server.base_url(),
        ledger_id,
        cash,
        sid
    );
    post_form(
        server,
        cookie,
        &format!("{base}/clear"),
        &[("bank_line_id", &l1.to_string()), ("confirm", "yes")],
    )
    .await;
    let resp = post_form(server, cookie, &format!("{base}/finish"), &[]).await;
    assert!(resp.status().is_redirection());
    sid
}

#[tokio::test]
async fn unreconcile_without_reason_returns_400() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec3", "rec3@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let sid = closed_session(&server, &pool, &cookie, ledger_id, cash).await;

    for fields in [&[][..], &[("reason", "")][..], &[("reason", "oops")][..]] {
        let resp = post_form(
            &server,
            &cookie,
            &format!(
                "{}/ledgers/{}/reconcile/{}/sessions/{}/unreconcile",
                server.base_url(),
                ledger_id,
                cash,
                sid
            ),
            fields,
        )
        .await;
        assert_eq!(resp.status(), 400, "reasonless unreconcile is rejected");
    }
    assert_eq!(session_status(&pool, sid).await, "closed");
}

#[tokio::test]
async fn unreconcile_with_reason_reopens_and_audits() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec4", "rec4@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let sid = closed_session(&server, &pool, &cookie, ledger_id, cash).await;

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reconcile/{}/sessions/{}/unreconcile",
            server.base_url(),
            ledger_id,
            cash,
            sid
        ),
        &[("reason", "correction: wrong statement period")],
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "reasoned unreconcile reopens"
    );
    assert_eq!(session_status(&pool, sid).await, "open");

    // Lines return to uncleared.
    let (cleared,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM rec_lines WHERE session_id = $1 AND cleared")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cleared, 0);

    // Audit row written.
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_entries WHERE entity_type = 'rec_session' AND entity_id = $1 AND action = 'reconcile_session_unreconcile'",
    )
    .bind(sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1);
}

// ─── Opening balance carries forward ─────────────────────────────────────────

#[tokio::test]
async fn opening_balance_carries_forward_from_prior_close() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec5", "rec5@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;

    // March session closed at 12,400.00.
    let l1 = insert_line(
        &pool,
        ledger_id,
        cash,
        "2026-03-15",
        "MARCH LINE",
        Decimal::new(1_240_000, 2),
    )
    .await;
    let (_s, march) =
        create_session(&server, &cookie, ledger_id, cash, "2026-03-31", "12400.00").await;
    let march = march.expect("march session");
    let base = format!(
        "{}/ledgers/{}/reconcile/{}/sessions/{}",
        server.base_url(),
        ledger_id,
        cash,
        march
    );
    post_form(
        &server,
        &cookie,
        &format!("{base}/clear"),
        &[("bank_line_id", &l1.to_string()), ("confirm", "1")],
    )
    .await;
    assert!(post_form(&server, &cookie, &format!("{base}/finish"), &[])
        .await
        .status()
        .is_redirection());

    // April opens at 12,400.00 automatically.
    let (_s, april) =
        create_session(&server, &cookie, ledger_id, cash, "2026-04-30", "13000.00").await;
    let april = april.expect("april session");
    let (opening,): (Decimal,) =
        sqlx::query_as("SELECT opening_balance FROM rec_sessions WHERE id = $1")
            .bind(april)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(opening, Decimal::new(1_240_000, 2));

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reconcile/{}/sessions/{}",
            server.base_url(),
            ledger_id,
            cash,
            april
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET session page");
    let body = resp.text().await.expect("body");
    assert!(body.contains("12400"), "page shows carried opening: {body}");
    assert!(
        body.contains("Difference"),
        "page shows difference indicator"
    );
}

// ─── 1.8 E2E: import → clear → finish → lock ──────────────────────────────────

#[tokio::test]
async fn e2e_import_clear_finish_locks() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec6", "rec6@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;

    // Import a CSV statement through the reconcile import route.
    let csv =
        "date,description,amount\n2026-08-21,ACME GMBH,-42.50\n2026-08-22,CLIENT PAY,1000.00\n";
    let part = reqwest::multipart::Part::text(csv.to_string()).file_name("stmt.csv".to_string());
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/reconcile/{}/import",
            server.base_url(),
            ledger_id,
            cash
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .multipart(form)
        .send()
        .await
        .expect("import");
    assert!(
        resp.status().is_redirection(),
        "import redirects, got {}",
        resp.status()
    );

    // Close = 1000.00 - 42.50 = 957.50.
    let (_s, sid) = create_session(&server, &cookie, ledger_id, cash, "2026-08-31", "957.50").await;
    let sid = sid.expect("session created");
    let base = format!(
        "{}/ledgers/{}/reconcile/{}/sessions/{}",
        server.base_url(),
        ledger_id,
        cash,
        sid
    );

    let ids: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM bank_statement_lines WHERE ledger_id = $1 AND account_id = $2",
    )
    .bind(ledger_id)
    .bind(cash)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ids.len(), 2);
    for (id,) in &ids {
        let resp = post_form(
            &server,
            &cookie,
            &format!("{base}/clear"),
            &[("bank_line_id", &id.to_string()), ("confirm", "on")],
        )
        .await;
        assert!(resp.status().is_redirection());
    }

    let resp = post_form(&server, &cookie, &format!("{base}/finish"), &[]).await;
    assert!(
        resp.status().is_redirection(),
        "zero-difference finish closes"
    );
    assert_eq!(session_status(&pool, sid).await, "closed");

    // Lock blocks unclear after the E2E close.
    let resp = post_form(
        &server,
        &cookie,
        &format!("{base}/unclear"),
        &[("bank_line_id", &ids[0].0.to_string())],
    )
    .await;
    assert_eq!(resp.status(), 409);
}

// ─── Rules surface as candidates requiring confirm ───────────────────────────

#[tokio::test]
async fn rule_suggestions_are_candidates_not_auto_clear() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("rec7", "rec7@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;

    insert_line(
        &pool,
        ledger_id,
        cash,
        "2026-08-21",
        "STARBUCKS STORE 12",
        Decimal::new(-4250, 2),
    )
    .await;
    let (_s, sid) = create_session(&server, &cookie, ledger_id, cash, "2026-08-31", "-42.50").await;
    let sid = sid.expect("session created");
    let base = format!(
        "{}/ledgers/{}/reconcile/{}/sessions/{}",
        server.base_url(),
        ledger_id,
        cash,
        sid
    );

    sqlx::query(
        r#"INSERT INTO reconciliation_rules (ledger_id, name, kind, priority, predicate, action)
           VALUES ($1, 'Starbucks flag', 'flag', 10,
                   '{"description_glob": "STARBUCKS%"}'::jsonb, '{"reason_text": "coffee"}'::jsonb)"#,
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .expect("insert rule");

    let resp = server
        .client()
        .get(format!("{base}/suggestions"))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("GET suggestions");
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.expect("json");
    let candidates = json["candidates"].as_array().expect("candidates array");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["rule"], "Starbucks flag");

    // The candidate is NOT cleared until the user confirms.
    let (cleared,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM rec_lines WHERE session_id = $1 AND cleared")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cleared, 0);
}
