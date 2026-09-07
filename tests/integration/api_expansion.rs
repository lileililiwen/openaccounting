//! Integration tests for `api-v2-coverage`: new resource endpoints,
//! durable idempotency, cursor pagination, per-token rate limits.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn issue_for(pool: &PgPool, email: &str) -> String {
    issue_for_with_id(pool, email).await.0
}

async fn issue_for_with_id(pool: &PgPool, email: &str) -> (String, Uuid) {
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap();
    let issued = openaccounting::auth::api_token::issue_token(pool, user_id, "test")
        .await
        .unwrap();
    (issued.plaintext, issued.id)
}

struct Ctx {
    server: TestServer,
    ledger_id: Uuid,
    owner_email: String,
    owner_token: String,
    owner_token_id: Uuid,
    viewer_token: String,
    editor_token: String,
    cash: Uuid,
    sales: Uuid,
}

async fn setup() -> Ctx {
    let server = TestServer::new().await;
    let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
    let owner_email = format!("owner-{suffix}@example.com");
    let viewer_email = format!("viewer-{suffix}@example.com");
    let editor_email = format!("editor-{suffix}@example.com");

    server
        .bootstrap_user(&owner_email, &owner_email, PASSWORD)
        .await;
    server
        .bootstrap_user(&viewer_email, &viewer_email, PASSWORD)
        .await;
    server
        .bootstrap_user(&editor_email, &editor_email, PASSWORD)
        .await;

    // Owner creates a ledger via the API.
    let (owner_token, owner_token_id) = issue_for_with_id(&server.db().pool(), &owner_email).await;
    // server.client() carries the X-OA-CSRF-Bypass default header.
    let resp = server
        .client()
        .post(format!("{}/api/v1/ledgers", server.base_url()))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {owner_token}"),
        )
        .json(&json!({
            "name": format!("API Co {suffix}"),
            "base_currency": "USD",
            "timezone": "UTC",
            "basis": "accrual",
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    assert_eq!(status, 201, "ledger create body: {text}");
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    let ledger_id: Uuid = body["id"].as_str().unwrap().parse().unwrap();

    // The API ledger-create does not seed a chart (matches existing
    // api.rs tests) — insert two accounts directly.
    let pool = server.db().pool();
    sqlx::query(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Cash', 'ASSET', 'CURRENT_ASSET', 'USD'),
                ($1, 'Sales', 'INCOME', 'OPERATING_INCOME', 'USD')",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();
    let cash: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let sales: Uuid =
        sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Share with viewer + editor.
    let (viewer_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&viewer_email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (editor_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&editor_email)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ledger_members (ledger_id, user_id, role) VALUES ($1, $2, 'viewer')")
        .bind(ledger_id)
        .bind(viewer_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ledger_members (ledger_id, user_id, role) VALUES ($1, $2, 'editor')")
        .bind(ledger_id)
        .bind(editor_id)
        .execute(&pool)
        .await
        .unwrap();

    let viewer_token = issue_for(&pool, &viewer_email).await;
    let editor_token = issue_for(&pool, &editor_email).await;

    Ctx {
        server,
        ledger_id,
        owner_email,
        owner_token,
        owner_token_id,
        viewer_token,
        editor_token,
        cash,
        sales,
    }
}

impl Ctx {
    fn base(&self) -> String {
        self.server.base_url().to_string()
    }
    fn auth(&self, token: &str) -> String {
        format!("Bearer {token}")
    }
}

// ── Invoices ────────────────────────────────────────────────────────────

#[tokio::test]
async fn invoice_create_mark_paid_void_round_trip() {
    let ctx = setup().await;
    let pool = ctx.server.db().pool();

    // Contact first.
    let resp = ctx
        .server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{}/contacts",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .json(&json!({ "name": "ACME", "kind": "customer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let contact: serde_json::Value = resp.json().await.unwrap();
    let contact_id = contact["id"].as_str().unwrap();

    // Create invoice.
    let resp = ctx
        .server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{}/invoices",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .json(&json!({
            "contact_id": contact_id,
            "kind": "receivable",
            "invoice_date": "2026-08-01",
            "due_date": "2026-08-31",
            "lines": [
                { "description": "Widget", "quantity": 2, "unit_price": "50.00" },
                { "description": "Setup", "quantity": 1, "unit_price": "25.50" }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let inv: serde_json::Value = resp.json().await.unwrap();
    let invoice_id = inv["id"].as_str().unwrap().to_string();
    assert_eq!(inv["total"].as_str(), Some("125.50"));

    // Show includes lines.
    let resp = ctx
        .server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{}/invoices/{invoice_id}",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .send()
        .await
        .unwrap();
    let shown: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(shown["lines"].as_array().unwrap().len(), 2);

    // Mark paid → status paid; UI-visible via invoices table.
    let resp = ctx
        .server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{}/invoices/{invoice_id}/mark-paid",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.editor_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let status: (String,) = sqlx::query_as("SELECT status FROM invoices WHERE id = $1")
        .bind(Uuid::parse_str(&invoice_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status.0, "paid");

    // Void after paid → 404 (already settled).
    let resp = ctx
        .server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{}/invoices/{invoice_id}/void",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── Role matrix ─────────────────────────────────────────────────────────

#[tokio::test]
async fn viewer_tokens_cannot_write_any_resource() {
    let ctx = setup().await;
    let write_endpoints: Vec<(String, serde_json::Value)> = vec![
        (
            format!("{}/api/v1/ledgers/{}/contacts", ctx.base(), ctx.ledger_id),
            json!({ "name": "X", "kind": "customer" }),
        ),
        (
            format!("{}/api/v1/ledgers/{}/invoices", ctx.base(), ctx.ledger_id),
            json!({}),
        ),
        (
            format!("{}/api/v1/ledgers/{}/payments", ctx.base(), ctx.ledger_id),
            json!({}),
        ),
        (
            format!("{}/api/v1/ledgers/{}/budgets", ctx.base(), ctx.ledger_id),
            json!({}),
        ),
    ];
    for (url, body) in write_endpoints {
        let resp = ctx
            .server
            .client()
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.viewer_token))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 403, "POST {url} must be 403 for viewers");
        let problem: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(problem["status"].as_u64(), Some(403), "RFC 7807 body");
    }

    // Viewer can still read.
    let resp = ctx
        .server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{}/contacts",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.viewer_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Stranger (no membership) gets 404, not 403 — no existence leak.
    let stranger_email = "stranger-api@example.com";
    ctx.server
        .bootstrap_user(stranger_email, stranger_email, PASSWORD)
        .await;
    let stranger_token = issue_for(&ctx.server.db().pool(), stranger_email).await;
    let resp = ctx
        .server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{}/contacts",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&stranger_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── Durable idempotency ─────────────────────────────────────────────────

async fn post_transaction(
    ctx: &Ctx,
    token: &str,
    key: Option<&str>,
    amount: &str,
) -> reqwest::Response {
    let mut req = ctx
        .server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{}/transactions",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(token))
        .json(&json!({
            "date": "2026-08-20",
            "description": "idem test",
            "lines": [
                { "account_id": ctx.cash, "direction": "DEBIT", "amount": amount },
                { "account_id": ctx.sales, "direction": "CREDIT", "amount": amount }
            ]
        }));
    if let Some(k) = key {
        req = req.header("Idempotency-Key", k);
    }
    req.send().await.unwrap()
}

#[tokio::test]
async fn idempotency_replays_across_restart_and_rejects_key_reuse() {
    let ctx = setup().await;
    let pool = ctx.server.db().pool();

    // First call creates.
    let r1 = post_transaction(&ctx, &ctx.owner_token, Some("key-restart-1"), "10.00").await;
    assert_eq!(r1.status(), 201);
    let body1 = r1.text().await.unwrap();

    // Simulate a restart: the old in-process cache is gone entirely —
    // the DB row is the only state. Retry with the same key + body.
    let r2 = post_transaction(&ctx, &ctx.owner_token, Some("key-restart-1"), "10.00").await;
    assert_eq!(r2.status(), 201, "replay returns the stored status");
    assert_eq!(
        r2.text().await.unwrap(),
        body1,
        "replay returns stored body"
    );

    // Exactly one transaction was created.
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND description = 'idem test'",
    )
    .bind(ctx.ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 1);

    // Same key, different body → 422, nothing executed.
    let mut req = ctx
        .server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{}/transactions",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .header("Idempotency-Key", "key-restart-1")
        .json(&json!({
            "date": "2026-08-21",
            "description": "different",
            "lines": [
                { "account_id": ctx.cash, "direction": "DEBIT", "amount": "99.00" },
                { "account_id": ctx.sales, "direction": "CREDIT", "amount": "99.00" }
            ]
        }));
    let r3 = req.send().await.unwrap();
    assert_eq!(r3.status(), 422);
    let n2: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND description = 'different'",
    )
    .bind(ctx.ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n2.0, 0);
}

// ── Cursor pagination ───────────────────────────────────────────────────

#[tokio::test]
async fn cursor_walk_visits_each_row_exactly_once_and_forges_fail() {
    let ctx = setup().await;
    let pool = ctx.server.db().pool();

    // Seed 120 transactions directly (fast) — dates staggered for order.
    for i in 0..120 {
        sqlx::query(
            "INSERT INTO transactions (ledger_id, txn_date, description, currency, kind, created_by)
             VALUES ($1, $2, $3, 'USD', 'standard',
                     (SELECT owner_id FROM ledgers WHERE id = $1))",
        )
        .bind(ctx.ledger_id)
        .bind(NaiveDate::from_ymd_opt(2026, 1, (i % 28) + 1).unwrap())
        .bind(format!("seed-{i:03}"))
        .execute(&pool)
        .await
        .unwrap();
    }

    // Walk with limit=50 → pages of 50/50/20.
    let mut seen = Vec::new();
    let mut url = format!(
        "{}/api/v1/ledgers/{}/transactions?limit=50",
        ctx.base(),
        ctx.ledger_id
    );
    loop {
        let resp = ctx
            .server
            .client()
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let link_header = {
            let headers = resp.headers().clone();
            headers
                .get(reqwest::header::LINK)
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        };
        let page: serde_json::Value = resp.json().await.unwrap();
        let rows = page["data"].as_array().unwrap();
        for r in rows {
            seen.push(r["description"].as_str().unwrap().to_string());
        }
        // Follow rel="next".
        match link_header {
            Some(l) => {
                let start = l.find('<').unwrap() + 1;
                let end = l.find('>').unwrap();
                url = format!("{}{}", ctx.base(), &l[start..end]);
            }
            None => break,
        }
    }
    let (total_expected,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
            .bind(ctx.ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(seen.len() as i64, total_expected);
    let unique = seen.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), seen.len(), "no duplicates across pages");

    // Forged cursor fails closed.
    let resp = ctx
        .server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{}/transactions?limit=50&cursor=999.deadbeef",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ── Rate limiting ───────────────────────────────────────────────────────

#[tokio::test]
async fn per_token_rate_limit_returns_429_with_retry_after() {
    let ctx = setup().await;

    // Deterministic throttle: pin THIS token's limit to 5/min via the
    // test-support override instead of racing the 60 s window under
    // parallel load.
    openaccounting::api::rate_limit::set_token_limit(ctx.owner_token_id, Some(5));

    let mut got_429 = false;
    let mut retry_after_seen = false;
    for i in 0..10 {
        let resp = ctx
            .server
            .client()
            .get(format!(
                "{}/api/v1/ledgers/{}/contacts",
                ctx.base(),
                ctx.ledger_id
            ))
            .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
            .send()
            .await
            .unwrap();
        if resp.status() == 429 {
            got_429 = true;
            retry_after_seen = resp.headers().contains_key(reqwest::header::RETRY_AFTER);
            break;
        }
        let _ = i;
    }
    assert!(got_429, "expected 429 after exhausting the window");
    assert!(retry_after_seen, "Retry-After header required");

    // A different token is unaffected.
    let resp = ctx
        .server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{}/contacts",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.viewer_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

// ── Cash-flow parity ────────────────────────────────────────────────────

#[tokio::test]
async fn api_cash_flow_matches_html_report() {
    let ctx = setup().await;
    let pool = ctx.server.db().pool();

    // Post a real cash movement: debit Cash 500 / credit Sales 500.
    sqlx::query(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, kind, created_by)
         VALUES ($1, '2026-03-10', 'cash sale', 'USD', 'standard',
                 (SELECT owner_id FROM ledgers WHERE id = $1)) RETURNING id",
    )
    .bind(ctx.ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let txn_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM transactions WHERE ledger_id = $1 AND description = 'cash sale'",
    )
    .bind(ctx.ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (acct, dir) in [(ctx.cash, "DEBIT"), (ctx.sales, "CREDIT")] {
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, direction, amount) VALUES ($1, $2, $3, $4)",
        )
        .bind(txn_id)
        .bind(acct)
        .bind(dir)
        .bind(rust_decimal::Decimal::new(50_000, 2))
        .execute(&pool)
        .await
        .unwrap();
    }

    // API figure.
    let resp = ctx
        .server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{}/reports/cash-flow?from=2026-01-01&to=2026-12-31",
            ctx.base(),
            ctx.ledger_id
        ))
        .header(reqwest::header::AUTHORIZATION, ctx.auth(&ctx.owner_token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let api: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        api["movement"]
            .as_str()
            .and_then(|v| v.parse::<rust_decimal::Decimal>().ok()),
        Some(rust_decimal::Decimal::new(50_000, 2))
    );
    assert_eq!(
        api["closing"]
            .as_str()
            .and_then(|v| v.parse::<rust_decimal::Decimal>().ok()),
        Some(rust_decimal::Decimal::new(50_000, 2))
    );

    // Log in as the ledger owner and read the HTML report.
    let resp = ctx
        .server
        .client()
        .post(format!("{}/login", ctx.base()))
        .form(&[
            ("email", ctx.owner_email.as_str()),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let html = ctx
        .server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/cash-flow?from=2026-01-01&to=2026-12-31",
            ctx.base(),
            ctx.ledger_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(html.status(), 200);
    let body = html.text().await.unwrap();
    assert!(body.contains("500.00"), "HTML report shows closing 500.00");
}
