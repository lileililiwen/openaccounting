//! Integration tests for reimbursement approval routing.

use crate::common::*;
use sqlx::PgPool;
use uuid::Uuid;

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Approve Co"),
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

async fn add_line(
    client: &reqwest::Client,
    base_url: &str,
    cookie: &str,
    ledger_id: Uuid,
    claim_id: Uuid,
    gl_account: Uuid,
    amount: &str,
) {
    let _ = post_form(
        client,
        &format!("{base_url}/ledgers/{ledger_id}/reimbursements/{claim_id}/lines"),
        cookie,
        &[
            ("txn_date", "2026-08-14"),
            ("description", "Conference"),
            ("amount", amount),
            ("gl_account_id", &gl_account.to_string()),
        ],
    )
    .await;
}

async fn submit(
    client: &reqwest::Client,
    base_url: &str,
    cookie: &str,
    ledger_id: Uuid,
    claim_id: Uuid,
) {
    let _ = post_form(
        client,
        &format!("{base_url}/ledgers/{ledger_id}/reimbursements/{claim_id}/submit"),
        cookie,
        &[],
    )
    .await;
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

async fn create_policies(server: &TestServer, cookie: &str, ledger_id: Uuid) {
    for (name, min, role, level) in [
        ("L1 Admin", "0", "Admin", "1"),
        ("L2 Accountant", "5000", "Accountant", "2"),
    ] {
        let resp = post_form(
            server.client(),
            &format!(
                "{}/ledgers/{ledger_id}/approval-policies",
                server.base_url()
            ),
            cookie,
            &[
                ("name", name),
                ("min_amount", min),
                ("approver_role", role),
                ("level", level),
            ],
        )
        .await;
        assert_eq!(resp.status(), 303, "create policy {name}");
    }
}

/// Register a user on an independent client (so its session cookie is
/// not clobbered by the shared jar), grant them ledger access, and
/// optionally elevate their global role to `admin`. Returns the user's
/// session cookie, user id, and a dedicated client.
async fn setup_user(
    server: &TestServer,
    pool: &PgPool,
    username: &str,
    email: &str,
    ledger_id: Uuid,
    member_role: Option<&str>,
    global_admin: bool,
) -> (String, Uuid, reqwest::Client) {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("build client");
    let pw = "correct horse battery staple";
    let resp = client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", username),
            ("password", pw),
            ("password_confirm", pw),
        ])
        .send()
        .await
        .expect("register");
    let status = resp.status();
    if !status.is_success() && status.as_u16() != 303 {
        let body = resp.text().await.unwrap_or_default();
        panic!("register failed for {username}: {status} body={body}");
    }
    let resp = client
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", pw), ("next", "/ledgers")])
        .send()
        .await
        .expect("login");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie");
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE username = $1")
        .bind(username)
        .fetch_one(pool)
        .await
        .expect("user exists");
    if let Some(role) = member_role {
        sqlx::query(
            r#"INSERT INTO ledger_members (ledger_id, user_id, role)
               VALUES ($1, $2, $3)"#,
        )
        .bind(ledger_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .expect("add member");
    }
    if global_admin {
        sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await
            .expect("set admin");
    }
    (cookie, user_id, client)
}

async fn claim_status(pool: &PgPool, claim_id: Uuid) -> String {
    let (s,): (String,) = sqlx::query_as("SELECT status FROM reimbursement_claims WHERE id = $1")
        .bind(claim_id)
        .fetch_one(pool)
        .await
        .expect("claim exists");
    s
}

async fn approve(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    claim_id: Uuid,
    level: i32,
) -> reqwest::Response {
    post_form(
        server.client(),
        &format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}/approve?level={level}",
            server.base_url()
        ),
        cookie,
        &[],
    )
    .await
}

#[tokio::test]
async fn http_claim_above_threshold_requires_two_levels() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let pool = &pool;
    let alice = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(pool, ledger_id, "Other Expense").await;
    create_policies(&server, &alice, ledger_id).await;
    // Bob is the Admin approver (member + global admin).
    let (bob_cookie, _bob, _bob_client) = setup_user(
        &server,
        pool,
        "bob",
        "bob@example.com",
        ledger_id,
        Some("editor"),
        true,
    )
    .await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        "Big Trip",
    )
    .await;
    add_line(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
        other,
        "12000.00",
    )
    .await;
    submit(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
    )
    .await;
    // Bob approves level 1; level 2 is still required, so the claim
    // remains partially_approved.
    let resp = approve(&server, &bob_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 303, "bob approves L1");
    assert_eq!(claim_status(pool, claim_id).await, "partially_approved");
    // A claim below the second threshold requires only level 1.
    let small_id = create_claim(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        "Small",
    )
    .await;
    add_line(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        small_id,
        other,
        "300.00",
    )
    .await;
    submit(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        small_id,
    )
    .await;
    let resp = approve(&server, &bob_cookie, ledger_id, small_id, 1).await;
    assert_eq!(resp.status(), 303, "bob approves small L1");
    assert_eq!(claim_status(pool, small_id).await, "fully_approved");
}

#[tokio::test]
async fn http_two_levels_approved_moves_to_fully_approved_and_posts_gl() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let pool = &pool;
    let alice = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(pool, ledger_id, "Other Expense").await;
    let _payable = account_id(pool, ledger_id, "Employee Payable").await;
    create_policies(&server, &alice, ledger_id).await;
    let (bob_cookie, _bob, _bob_client) = setup_user(
        &server,
        pool,
        "bob",
        "bob@example.com",
        ledger_id,
        Some("editor"),
        true,
    )
    .await;
    let (carol_cookie, _carol, _carol_client) = setup_user(
        &server,
        pool,
        "carol",
        "carol@example.com",
        ledger_id,
        Some("editor"),
        false,
    )
    .await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        "Big Trip",
    )
    .await;
    add_line(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
        other,
        "12000.00",
    )
    .await;
    submit(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
    )
    .await;
    // Admin approves level 1.
    let resp = approve(&server, &bob_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 303, "bob approves L1");
    assert_eq!(claim_status(pool, claim_id).await, "partially_approved");
    // Accountant approves level 2 -> fully_approved.
    let resp = approve(&server, &carol_cookie, ledger_id, claim_id, 2).await;
    assert_eq!(resp.status(), 303, "carol approves L2");
    assert_eq!(claim_status(pool, claim_id).await, "fully_approved");
    // GL posting runs exactly once: one transaction, two postings.
    let (txn_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM transactions t
           WHERE t.ledger_id = $1 AND t.reference = (
             SELECT short_id FROM reimbursement_claims WHERE id = $2
           )"#,
    )
    .bind(ledger_id)
    .bind(claim_id)
    .fetch_one(pool)
    .await
    .expect("count");
    assert_eq!(txn_count, 1);
    let (posting_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND t.reference = (
             SELECT short_id FROM reimbursement_claims WHERE id = $2
           )"#,
    )
    .bind(ledger_id)
    .bind(claim_id)
    .fetch_one(pool)
    .await
    .expect("count");
    assert_eq!(posting_count, 2);
    // A further approve is rejected with 409.
    let resp = approve(&server, &carol_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn http_approve_is_idempotent_per_level() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let pool = &pool;
    let alice = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(pool, ledger_id, "Other Expense").await;
    create_policies(&server, &alice, ledger_id).await;
    let (bob_cookie, _bob, _bob_client) = setup_user(
        &server,
        pool,
        "bob",
        "bob@example.com",
        ledger_id,
        Some("editor"),
        true,
    )
    .await;
    let (carol_cookie, _carol, _carol_client) = setup_user(
        &server,
        pool,
        "carol",
        "carol@example.com",
        ledger_id,
        Some("editor"),
        false,
    )
    .await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        "Big Trip",
    )
    .await;
    add_line(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
        other,
        "12000.00",
    )
    .await;
    submit(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
    )
    .await;
    // Bob approves level 1 twice: the second is a no-op 200.
    let resp = approve(&server, &bob_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 303, "first L1 approve");
    let resp = approve(&server, &bob_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 200, "second L1 approve is a no-op");
    // A different user approving the same already-recorded level is
    // also a no-op.
    let resp = approve(&server, &carol_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 200, "other user same level is a no-op");
    assert_eq!(claim_status(pool, claim_id).await, "partially_approved");
}

#[tokio::test]
async fn http_author_cannot_self_approve_in_shared_ledger() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let pool = &pool;
    let alice = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(pool, ledger_id, "Other Expense").await;
    // Bob is an Admin on the ledger.
    let (_bob_cookie, _bob_id, _bob_client) = setup_user(
        &server,
        pool,
        "bob",
        "bob@example.com",
        ledger_id,
        Some("editor"),
        true,
    )
    .await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        "Trip",
    )
    .await;
    add_line(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
        other,
        "12.50",
    )
    .await;
    submit(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
    )
    .await;
    // Alice tries to approve her own claim -> 403.
    let resp = approve(&server, &alice, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 403);
    let body = resp.text().await.expect("body");
    assert!(
        body.contains("Authors cannot approve their own claims."),
        "body should contain the documented message; got: {body}"
    );
}

#[tokio::test]
async fn http_admin_sees_level_breakdown() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let pool = &pool;
    let alice = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let other = account_id(pool, ledger_id, "Other Expense").await;
    create_policies(&server, &alice, ledger_id).await;
    let (bob_cookie, _bob, _bob_client) = setup_user(
        &server,
        pool,
        "bob",
        "bob@example.com",
        ledger_id,
        Some("editor"),
        true,
    )
    .await;
    let claim_id = create_claim(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        "Big Trip",
    )
    .await;
    add_line(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
        other,
        "12000.00",
    )
    .await;
    submit(
        server.client(),
        server.base_url(),
        &alice,
        ledger_id,
        claim_id,
    )
    .await;
    let resp = approve(&server, &bob_cookie, ledger_id, claim_id, 1).await;
    assert_eq!(resp.status(), 303, "bob approves L1");
    assert_eq!(claim_status(pool, claim_id).await, "partially_approved");
    // Bob (approver) sees the per-level breakdown.
    let page = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &bob_cookie)
        .send()
        .await
        .expect("GET show");
    let body = page.text().await.expect("body");
    assert!(body.contains("Level 1"), "admin sees Level 1: {body}");
    assert!(body.contains("approved"), "admin sees approved: {body}");
    assert!(body.contains("Level 2"), "admin sees Level 2: {body}");
    assert!(body.contains("pending"), "admin sees pending: {body}");
    assert!(body.contains("bob"), "admin sees approver name: {body}");
    // Alice (author) sees only the aggregate.
    let page = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reimbursements/{claim_id}",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, alice)
        .send()
        .await
        .expect("GET show");
    let body = page.text().await.expect("body");
    assert!(
        body.contains("Awaiting 1 more approval"),
        "author sees aggregate: {body}"
    );
    assert!(
        !body.contains("Level 1"),
        "author must not see Level 1: {body}"
    );
    assert!(
        !body.contains("pending"),
        "author must not see per-level status: {body}"
    );
    assert!(
        !body.contains(">bob<"),
        "author must not see approver name: {body}"
    );
}
