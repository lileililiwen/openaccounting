//! Integration tests for `automation-platform`: job queue, recurring
//! templates, invoice reminders, outgoing webhooks, notification
//! channels.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use sha2::Sha256;
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";
const D: fn(i32, u32, u32) -> NaiveDate = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();

async fn bootstrap(server: &TestServer) -> (Uuid, Uuid, Uuid, Uuid) {
    let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
    let email = format!("auto-{suffix}@example.com");
    let client = reqwest::Client::builder()
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
        .unwrap();
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("username", format!("auto-{suffix}").as_str()),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    let cookie = {
        let resp = client
            .post(format!("{}/login", server.base_url()))
            .form(&[
                ("email", email.as_str()),
                ("password", PASSWORD),
                ("next", "/ledgers"),
            ])
            .send()
            .await
            .unwrap();
        resp.headers()
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
            .unwrap()
    };

    let pool = server.db().pool();
    let (uid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", format!("Auto Co {suffix}").as_str()),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();
    let cash: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let rent: Uuid = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Rent'",
    )
    .bind(ledger_id)
    .fetch_optional(&pool)
    .await
    .unwrap()
    {
        Some(id) => id,
        None => seed_account(&pool, ledger_id, "Rent", "EXPENSE", "OPERATING_EXPENSE").await,
    };
    (uid, ledger_id, cash, rent)
}

async fn seed_account(pool: &PgPool, ledger_id: Uuid, name: &str, t: &str, subtype: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, $2, $3, $4, 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .bind(name)
    .bind(t)
    .bind(subtype)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_template(
    pool: &PgPool,
    ledger_id: Uuid,
    debit: Uuid,
    credit: Uuid,
    next_date: NaiveDate,
) -> Uuid {
    let (tid,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transaction_templates
               (ledger_id, description, payee, frequency, next_date, is_active)
           VALUES ($1, 'Monthly rent', 'Landlord LLC', 'monthly', $2, TRUE)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(next_date)
    .fetch_one(pool)
    .await
    .unwrap();
    for (acct, dir) in [(debit, "DEBIT"), (credit, "CREDIT")] {
        sqlx::query(
            "INSERT INTO template_postings (template_id, account_id, direction, amount)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(tid)
        .bind(acct)
        .bind(dir)
        .bind(Decimal::new(250_000, 2))
        .execute(pool)
        .await
        .unwrap();
    }
    tid
}

// ── Queue mechanics ─────────────────────────────────────────────────────

#[tokio::test]
async fn queue_claim_is_exclusive_and_dead_letters_poison() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    for i in 0..20 {
        openaccounting::jobs::enqueue(
            &pool,
            "noop_kind_for_claim_test",
            serde_json::json!({ "i": i }),
            None,
            None,
        )
        .await
        .unwrap();
    }
    let first = openaccounting::jobs::claim_due(&pool, 50).await.unwrap();
    assert_eq!(first.len(), 20);
    let second = openaccounting::jobs::claim_due(&pool, 50).await.unwrap();
    assert!(second.is_empty(), "claimed jobs must not be re-claimable");

    // Unknown kind with max_attempts=1 dies immediately with the error.
    let jid =
        openaccounting::jobs::enqueue(&pool, "no_such_handler", serde_json::json!({}), None, None)
            .await
            .unwrap();
    sqlx::query("UPDATE jobs SET max_attempts = 1 WHERE id = $1")
        .bind(jid)
        .execute(&pool)
        .await
        .unwrap();
    let job = openaccounting::jobs::claim_due(&pool, 10).await.unwrap();
    assert_eq!(job.len(), 1);
    openaccounting::jobs::execute(&pool, job.into_iter().next().unwrap()).await;
    let (status, err): (String, Option<String>) =
        sqlx::query_as("SELECT status, last_error FROM jobs WHERE id = $1")
            .bind(jid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "dead");
    assert!(err.unwrap().contains("unknown job kind"));
}

// ── Recurring templates ─────────────────────────────────────────────────

#[tokio::test]
async fn scheduler_posts_due_templates_exactly_once_per_due_date() {
    let server = TestServer::new().await;
    let (_uid, ledger_id, cash, rent) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Due 2026-05-01 monthly; today (2026-08) makes four missed months.
    seed_template(&pool, ledger_id, rent, cash, D(2026, 5, 1)).await;

    openaccounting::jobs::template_run::scan_and_run(&pool)
        .await
        .unwrap();

    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind = 'recurring'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 4, "one occurrence per missed month");

    // Re-run: idempotent, no new rows.
    openaccounting::jobs::template_run::scan_and_run(&pool)
        .await
        .unwrap();
    let n2: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind = 'recurring'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n2.0, 4);

    // Each posted transaction balances.
    let balanced: (bool,) = sqlx::query_as(
        r#"SELECT NOT EXISTS (
              SELECT 1 FROM postings p JOIN transactions t ON t.id = p.transaction_id
              WHERE t.ledger_id = $1 AND t.kind = 'recurring'
              GROUP BY p.transaction_id
              HAVING SUM(CASE WHEN direction='DEBIT' THEN amount ELSE -amount END) <> 0)"#,
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(balanced.0);
}

// ── Invoice reminders ───────────────────────────────────────────────────

async fn seed_overdue_invoice(pool: &PgPool, ledger_id: Uuid, days_overdue: i64) -> (Uuid, Uuid) {
    let (contact_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO contacts (ledger_id, name, email, kind)
         VALUES ($1, 'ACME Corp', '', 'customer') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let due = chrono::Utc::now().date_naive() - chrono::Duration::days(days_overdue);
    let (inv_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number,
                                 invoice_date, due_date, total, status)
           VALUES ($1, $2, 'receivable', 'INV-001', $3 - 30, $3, 500.00, 'open')
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .bind(due)
    .fetch_one(pool)
    .await
    .unwrap();
    (inv_id, contact_id)
}

#[tokio::test]
async fn overdue_reminders_fire_once_per_offset() {
    let server = TestServer::new().await;
    let (_uid, ledger_id, _cash, _rent) = bootstrap(&server).await;
    let pool = server.db().pool();

    seed_overdue_invoice(&pool, ledger_id, 8).await;

    openaccounting::jobs::invoice_reminders::scan(&pool)
        .await
        .unwrap();
    // Second run the same day must not duplicate.
    openaccounting::jobs::invoice_reminders::scan(&pool)
        .await
        .unwrap();

    let offsets: Vec<(i32,)> =
        sqlx::query_as("SELECT offset_day FROM invoice_reminders ORDER BY offset_day")
            .fetch_all(&pool)
            .await
            .unwrap();
    let offsets: Vec<i32> = offsets.into_iter().map(|(o,)| o).collect();
    assert_eq!(offsets, vec![1, 7], "day-14 reminder not yet due at 8 days");

    // In-app notification recorded for the owner.
    let n: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM in_app_notifications WHERE event = 'invoice_overdue'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(n.0 >= 1);
}

// ── Outgoing webhooks ───────────────────────────────────────────────────

/// Local HTTP receiver returning a scripted status sequence.
struct TestReceiver {
    addr: String,
    hits: Arc<AtomicUsize>,
    bodies: Arc<std::sync::Mutex<Vec<String>>>,
    sigs: Arc<std::sync::Mutex<Vec<String>>>,
    _handle: tokio::task::JoinHandle<()>,
}

impl Drop for TestReceiver {
    fn drop(&mut self) {
        self._handle.abort();
    }
}

async fn spawn_receiver(statuses: Vec<u16>) -> TestReceiver {
    use axum::routing::post as post_route;
    use axum::{Json, Router};
    let hits = Arc::new(AtomicUsize::new(0));
    let bodies = Arc::new(std::sync::Mutex::<Vec<String>>::new(Vec::new()));
    let sigs = Arc::new(std::sync::Mutex::<Vec<String>>::new(Vec::new()));
    let statuses = Arc::new(std::sync::Mutex::new(statuses));

    let h_hits = hits.clone();
    let h_bodies = bodies.clone();
    let h_sigs = sigs.clone();
    let app = Router::new().route(
        "/hook",
        post_route(move |headers: axum::http::HeaderMap, body: String| {
            let st = statuses.clone();
            let hits = h_hits.clone();
            let bodies = h_bodies.clone();
            let sigs = h_sigs.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                bodies.lock().unwrap().push(body);
                sigs.lock().unwrap().push(
                    headers
                        .get("X-OA-Signature")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_string(),
                );
                let code = {
                    let mut q = st.lock().unwrap();
                    let c = if q.len() > 1 { q.remove(0) } else { q[0] };
                    eprintln!("[dbg] receiver hit -> {c} (queue now {q:?})");
                    c
                };
                let status =
                    axum::http::StatusCode::from_u16(code).unwrap_or(axum::http::StatusCode::OK);
                (status, Json(serde_json::json!({ "status": code })))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestReceiver {
        addr: format!("http://{}", addr),
        hits,
        bodies,
        sigs,
        _handle: handle,
    }
}

fn verify_hmac(secret: &str, body: &str, presented: &str) -> bool {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    let expected = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    expected == presented
}

#[tokio::test]
async fn webhook_delivery_signs_retries_and_records_attempts() {
    std::env::set_var("WEBHOOK_ALLOW_INSECURE_HTTP", "true");
    let server = TestServer::new().await;
    let (_uid, ledger_id, _cash, _rent) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Receiver fails twice then succeeds.
    let rx = spawn_receiver(vec![500, 500, 200]).await;

    let secret = "whsec_test_secret_0123456789abcdef";
    let (sub_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO webhook_subscriptions (ledger_id, target_url, secret, events)
         VALUES ($1, $2, $3, ARRAY['invoice.paid']) RETURNING id",
    )
    .bind(ledger_id)
    .bind(format!("{}/hook", rx.addr))
    .bind(secret)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Emit an invoice.paid event through the real fan-out.
    openaccounting::jobs::events::emit(
        &pool,
        ledger_id,
        "invoice.paid",
        serde_json::json!({ "invoice_id": Uuid::new_v4() }),
    )
    .await;

    // Drain the queue until the delivery succeeds. Retries are
    // scheduled with production backoff, so pull them forward.
    for _ in 0..10 {
        sqlx::query(
            "UPDATE jobs SET run_at = now() - INTERVAL '1 second' WHERE status = 'pending'",
        )
        .execute(&pool)
        .await
        .unwrap();
        openaccounting::jobs::run_due(&pool, 5).await.unwrap();
        if rx.hits.load(Ordering::SeqCst) >= 3 {
            break;
        }
    }

    assert_eq!(
        rx.hits.load(Ordering::SeqCst),
        3,
        "two failures + one success"
    );
    let attempts: Vec<(i32, Option<bool>, Option<i32>)> =
        sqlx::query_as("SELECT attempt, ok, status_code FROM webhook_deliveries ORDER BY attempt")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        attempts.iter().all(|(_, ok, _)| ok.is_some()),
        "unset attempt rows: {attempts:?} hits={} bodies={:?} sigs={:?}",
        rx.hits.load(Ordering::SeqCst),
        rx.bodies.lock().unwrap(),
        rx.sigs.lock().unwrap()
    );
    assert_eq!(attempts.len(), 3);
    assert_eq!(attempts[0], (0, Some(false), Some(500)));
    assert_eq!(attempts[1], (1, Some(false), Some(500)));
    assert_eq!(attempts[2], (2, Some(true), Some(200)));

    // The body verifies against the subscription secret.
    let body = rx.bodies.lock().unwrap()[0].clone();
    let sig = rx.sigs.lock().unwrap()[0].clone();
    assert!(
        verify_hmac(secret, &body, &sig),
        "HMAC over raw body verifies"
    );
    let envelope: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(envelope["type"], "invoice.paid");
    assert!(envelope["occurred_at"].is_string());
    assert_eq!(
        envelope["data"]["subscription_check"],
        serde_json::Value::Null
    );

    // Secret is never rendered back by the management UI.
    let page = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/webhooks/subscriptions",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), 200);
    assert!(!page.text().await.unwrap().contains(secret));

    let _ = sub_id;
    std::env::remove_var("WEBHOOK_ALLOW_INSECURE_HTTP");
}

#[tokio::test]
async fn subscription_requires_https_and_owner_role() {
    let server = TestServer::new().await;
    let (_uid, ledger_id, _cash, _rent) = bootstrap(&server).await;

    // http:// target rejected.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/webhooks/subscriptions",
            server.base_url()
        ))
        .form(&[
            ("target_url", "http://example.com/hook"),
            ("events", "invoice.paid"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Valid https target accepted.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/webhooks/subscriptions",
            server.base_url()
        ))
        .form(&[
            ("target_url", "https://example.com/hook"),
            ("events", "invoice.paid"),
            ("events", "transaction.posted"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Copy your signing secret"),
        "secret shown once after create"
    );

    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM webhook_subscriptions")
        .fetch_one(&server.db().pool())
        .await
        .unwrap();
    assert_eq!(n.0, 1);
}

/// Property-style: random event filters enqueue exactly the matching set.
#[tokio::test]
async fn property_event_filtering_is_exact() {
    let server = TestServer::new().await;
    let (_uid, ledger_id, _cash, _rent) = bootstrap(&server).await;
    let pool = server.db().pool();

    let all = openaccounting::jobs::events::EVENT_TYPES;
    let mut seed = 0xbeef_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    let mut expected_total = 0usize;
    for _ in 0..20 {
        let mut chosen: Vec<String> = all
            .iter()
            .filter(|_| next() % 2 == 0)
            .map(|s| s.to_string())
            .collect();
        if chosen.is_empty() {
            continue;
        }
        chosen.sort();
        expected_total += chosen.len();
        sqlx::query(
            "INSERT INTO webhook_subscriptions (ledger_id, target_url, secret, events)
             VALUES ($1, 'https://example.com/x', 'whsec_x', $2)",
        )
        .bind(ledger_id)
        .bind(&chosen)
        .execute(&pool)
        .await
        .unwrap();
    }

    for et in all {
        openaccounting::jobs::events::emit(&pool, ledger_id, et, serde_json::json!({})).await;
    }

    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM webhook_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        n.0 as usize, expected_total,
        "deliveries match filter matrix"
    );
}
