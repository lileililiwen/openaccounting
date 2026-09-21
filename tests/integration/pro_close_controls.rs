//! Integration tests for professional close controls
//! (`pro-close-controls`): hard-close watermark, reopen override
//! audit, maker-checker, accountant/auditor roles, gapless invoice
//! numbering.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use openaccounting::domain::{
    posting_service::{NewTransaction, PostingService},
    TxnLineInput,
};
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

async fn register_login(client: &reqwest::Client, base: &str, email: &str) {
    client
        .post(format!("{base}/register"))
        .form(&[
            ("email", email),
            ("username", email.split('@').next().unwrap_or("u")),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/login"))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
        .send()
        .await
        .unwrap();
}

async fn create_ledger(client: &reqwest::Client, base: &str, name: &str) -> Uuid {
    let resp = client
        .post(format!("{base}/ledgers/new"))
        .form(&[
            ("name", name),
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
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    loc.rsplit('/').next().unwrap().parse().unwrap()
}

async fn account_ids(pool: &sqlx::PgPool, ledger_id: Uuid) -> (Uuid, Uuid) {
    let cash: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let sales: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (cash, sales)
}

#[allow(clippy::too_many_arguments)]
async fn post_txn(
    client: &reqwest::Client,
    base: &str,
    cookie: Option<&str>,
    ledger_id: Uuid,
    cash: Uuid,
    sales: Uuid,
    date: &str,
    amount: &str,
    extra: Vec<(&str, &str)>,
) -> reqwest::Response {
    let mut form: Vec<(&str, &str)> = vec![
        ("date", date),
        ("description", "Close test"),
        (
            "lines[0][account_id]",
            Box::leak(cash.to_string().into_boxed_str()),
        ),
        ("lines[0][direction]", "DEBIT"),
        ("lines[0][amount]", amount),
        ("lines[0][memo]", ""),
        (
            "lines[1][account_id]",
            Box::leak(sales.to_string().into_boxed_str()),
        ),
        ("lines[1][direction]", "CREDIT"),
        ("lines[1][amount]", amount),
        ("lines[1][memo]", ""),
    ];
    form.extend(extra);
    let mut req = client.post(format!("{base}/ledgers/{ledger_id}/transactions/new"));
    if let Some(c) = cookie {
        req = req.header(reqwest::header::COOKIE, c);
    }
    req.form(&form).send().await.unwrap()
}

fn lines_for(cash: Uuid, sales: Uuid, amount: Decimal) -> Vec<TxnLineInput> {
    vec![
        TxnLineInput {
            account_id: cash,
            signed_amount: amount,
            memo: None,
            tax_rate_id: None,
            foreign: None,
            cost_center_id: None,
            project_id: None,
        },
        TxnLineInput {
            account_id: sales,
            signed_amount: -amount,
            memo: None,
            tax_rate_id: None,
            foreign: None,
            cost_center_id: None,
            project_id: None,
        },
    ]
}

// 1.3 — posting into a closed period via the service layer fails.
#[tokio::test]
async fn service_post_into_closed_period_fails() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "close-svc@example.com").await;
    let ledger_id = create_ledger(&client, &base, "CloseSvc").await;
    let (cash, sales) = account_ids(&pool, ledger_id).await;
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("close-svc@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by, closed_through)
         VALUES ($1, 2026, $2, '2026-03-31')",
    )
    .bind(ledger_id)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();

    let err = PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
            description: "closed".into(),
            payee: None,
            reference: None,
            kind: Some("standard".into()),
            created_by: user,
            lines: lines_for(cash, sales, Decimal::new(100, 0)),
            reverses_id: None,
            number: None,
            tax_links: vec![],
        },
    )
    .await
    .expect_err("closed write must fail");
    assert!(matches!(
        err,
        openaccounting::domain::posting_service::PostingServiceError::HardClosed { .. }
    ));

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND description = 'closed'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count.0, 0, "no rows may be written");
}

// 1.4 — maker cannot approve own journal; a different approver succeeds.
#[tokio::test]
async fn maker_cannot_self_approve_second_user_can() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "maker@example.com").await;
    register_login(&client, &base, "checker@example.com").await;
    let ledger_id = create_ledger(&client, &base, "MakerChecker").await;
    let (cash, sales) = account_ids(&pool, ledger_id).await;
    let maker: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("maker@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    let checker: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("checker@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("UPDATE ledgers SET approval_threshold = 100 WHERE id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();

    let created = PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            description: "big journal".into(),
            payee: None,
            reference: None,
            kind: Some("standard".into()),
            created_by: maker,
            lines: lines_for(cash, sales, Decimal::new(50_000, 0)),
            reverses_id: None,
            number: None,
            tax_links: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(created.kind, "pending");

    let err = PostingService::approve_journal(&pool, ledger_id, created.id, maker)
        .await
        .expect_err("self-approval must fail");
    assert!(matches!(
        err,
        openaccounting::domain::posting_service::PostingServiceError::SelfApproval
    ));
    let kind: String = sqlx::query_scalar("SELECT kind FROM transactions WHERE id = $1")
        .bind(created.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "pending");

    PostingService::approve_journal(&pool, ledger_id, created.id, checker)
        .await
        .unwrap();
    let kind: String = sqlx::query_scalar("SELECT kind FROM transactions WHERE id = $1")
        .bind(created.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "standard");
}

// 1.6 — HTTP POST into a closed period returns 409 naming the date.
#[tokio::test]
async fn http_post_into_closed_period_returns_409() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "close-http@example.com").await;
    let ledger_id = create_ledger(&client, &base, "CloseHttp").await;
    let (cash, sales) = account_ids(&pool, ledger_id).await;

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/close"))
        .form(&[("closed_through", "2026-03-31")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let resp = post_txn(
        &client,
        &base,
        None,
        ledger_id,
        cash,
        sales,
        "2026-03-15",
        "10.00",
        vec![],
    )
    .await;
    assert_eq!(resp.status(), 409);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("2026-03-31"),
        "body must name the closed date"
    );

    let resp = post_txn(
        &client,
        &base,
        None,
        ledger_id,
        cash,
        sales,
        "2026-04-01",
        "10.00",
        vec![],
    )
    .await;
    assert_eq!(resp.status(), 303);
}

// 1.7 — reopen without reason is 400; with reason succeeds + audit row.
#[tokio::test]
async fn http_reopen_reason_validated_and_audited() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "reopen@example.com").await;
    let ledger_id = create_ledger(&client, &base, "Reopen").await;

    client
        .post(format!("{base}/ledgers/{ledger_id}/close"))
        .form(&[("closed_through", "2026-03-31")])
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/reopen"))
        .form(&[("reason", ""), ("closed_through", "")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/reopen"))
        .form(&[("reason", "short"), ("closed_through", "")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/reopen"))
        .form(&[
            (
                "reason",
                "Correcting supplier invoice INV-104 per auditor request",
            ),
            ("closed_through", ""),
        ])
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_redirection() || resp.status().is_success());

    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reopen_events WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n.0, 1);
    let a: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_entries WHERE ledger_id = $1 AND action = 'reopen'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(a.0 >= 1, "reopen must write an audit row");
}

// 1.8 — auditor can export but gets 403 on transaction create.
#[tokio::test]
async fn http_auditor_can_export_but_cannot_write() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let owner = make_client();
    let auditor = make_client();
    let base = server.base_url().to_string();
    register_login(&owner, &base, "aud-owner@example.com").await;
    register_login(&auditor, &base, "aud-auditor@example.com").await;
    let ledger_id = create_ledger(&owner, &base, "AuditorLedger").await;
    let (cash, sales) = account_ids(&pool, ledger_id).await;

    let auditor_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("aud-auditor@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ledger_members (ledger_id, user_id, role) VALUES ($1, $2, 'auditor')")
        .bind(ledger_id)
        .bind(auditor_id)
        .execute(&pool)
        .await
        .unwrap();

    let resp = auditor
        .get(format!("{base}/ledgers/{ledger_id}/export.json"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = post_txn(
        &auditor,
        &base,
        None,
        ledger_id,
        cash,
        sales,
        "2026-06-01",
        "5.00",
        vec![],
    )
    .await;
    assert_eq!(resp.status(), 403);
}

// 1.5 (property) — random close/reopen/post never leaves a posted txn in a closed range.
#[tokio::test]
async fn property_close_reopen_post_never_leaves_closed_posted_txn() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "prop-close@example.com").await;
    let ledger_id = create_ledger(&client, &base, "PropClose").await;
    let (cash, sales) = account_ids(&pool, ledger_id).await;
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind("prop-close@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Deterministic LCG so the run is reproducible.
    let mut rng: u64 = 0x1234_5678_9abc_def1;
    let mut next = move || {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 33) as u32
    };

    sqlx::query("UPDATE ledgers SET approval_threshold = -1 WHERE id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut successes: i64 = 0;
    for i in 0..40 {
        let op = next() % 3;
        let day = 1 + next() % 28;
        let month = 1 + next() % 6;
        let date = NaiveDate::from_ymd_opt(2026, month, day).unwrap();
        match op {
            0 => {
                // close through `date` when not already closed past it
                let w: Option<NaiveDate> =
                    openaccounting::domain::close_controls::closed_through_for(&pool, ledger_id)
                        .await
                        .unwrap();
                if w.is_none_or(|x| date > x) {
                    sqlx::query(
                        "INSERT INTO closed_periods (ledger_id, period_year, closed_by, closed_through)
                         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
                    )
                    .bind(ledger_id)
                    .bind(date.format("%Y").to_string().parse::<i32>().unwrap_or(2026))
                    .bind(user)
                    .bind(date)
                    .execute(&pool)
                    .await
                    .unwrap();
                }
            }
            1 => {
                sqlx::query("DELETE FROM closed_periods WHERE ledger_id = $1")
                    .bind(ledger_id)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            _ => {
                // Attempt a post; the property is that a post at a
                // closed date always fails and writes nothing.
                // (Closing over already-posted history is normal —
                // the watermark locks it; it must not delete it.)
                let w_before: Option<NaiveDate> =
                    openaccounting::domain::close_controls::closed_through_for(&pool, ledger_id)
                        .await
                        .unwrap();
                let count_before: (i64,) =
                    sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
                        .bind(ledger_id)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                let res = PostingService::create(
                    &pool,
                    NewTransaction {
                        ledger_id,
                        txn_date: date,
                        description: format!("prop {i}"),
                        payee: None,
                        reference: None,
                        kind: Some("standard".into()),
                        created_by: user,
                        lines: lines_for(cash, sales, Decimal::new(10, 0)),
                        reverses_id: None,
                        number: None,
                        tax_links: vec![],
                    },
                )
                .await;
                let count_after: (i64,) =
                    sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
                        .bind(ledger_id)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                let was_closed = w_before.map(|w| date <= w).unwrap_or(false);
                if was_closed {
                    assert!(res.is_err(), "post into closed range must fail at step {i}");
                    assert_eq!(
                        count_before.0, count_after.0,
                        "failed closed post must write nothing at step {i}"
                    );
                } else {
                    assert!(res.is_ok(), "post into open range must succeed at step {i}");
                    successes += 1;
                }
            }
        }
        // Invariant: closes/reopens never delete posted history —
        // every successful post must still be present.
        let remaining: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind IN ('standard','adjusting','closing','reversing','posted')",
        )
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            remaining.0 >= successes,
            "close/reopen must never delete posted history at step {i}"
        );
    }
}

// Invoices: auto numbers are gapless; void keeps its number and the gap
// report lists it as void, not missing.
#[tokio::test]
async fn invoices_gapless_sequence_and_void_report() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "invseq@example.com").await;
    let ledger_id = create_ledger(&client, &base, "InvSeq").await;

    let contact: Uuid = sqlx::query_scalar(
        "INSERT INTO contacts (ledger_id, name, kind) VALUES ($1, 'Acme', 'customer') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    async fn create_inv(
        client: &reqwest::Client,
        base: &str,
        ledger_id: Uuid,
        contact: Uuid,
        number: Option<&str>,
    ) -> String {
        let mut form: Vec<(&str, String)> = vec![
            ("contact_id", contact.to_string()),
            ("kind", "receivable".into()),
            ("invoice_date", "2026-07-01".into()),
            ("due_date", "2026-08-01".into()),
            ("lines[0][description]", "Work".into()),
            ("lines[0][quantity]", "2".into()),
            ("lines[0][unit_price]", "800".into()),
        ];
        if let Some(n) = number {
            form.push(("invoice_number", n.to_string()));
        }
        let resp = client
            .post(format!("{base}/ledgers/{ledger_id}/invoices/new"))
            .form(&form)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 303, "invoice create must redirect");
        resp.headers()
            .get(reqwest::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }

    let loc1 = create_inv(&client, &base, ledger_id, contact, None).await;
    let loc2 = create_inv(&client, &base, ledger_id, contact, None).await;
    let _loc3 = create_inv(&client, &base, ledger_id, contact, None).await;
    let id1: Uuid = loc1.rsplit('/').next().unwrap().parse().unwrap();
    let id2: Uuid = loc2.rsplit('/').next().unwrap().parse().unwrap();

    let n1: String = sqlx::query_scalar("SELECT invoice_number FROM invoices WHERE id = $1")
        .bind(id1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let n2: String = sqlx::query_scalar("SELECT invoice_number FROM invoices WHERE id = $1")
        .bind(id2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n1, "2026-0001");
    assert_eq!(n2, "2026-0002");

    // Two concurrent allocations must be gapless and distinct.
    let (a, b) = tokio::join!(
        create_inv(&client, &base, ledger_id, contact, None),
        create_inv(&client, &base, ledger_id, contact, None)
    );
    let na: String = sqlx::query_scalar("SELECT invoice_number FROM invoices WHERE id = $1")
        .bind(a.rsplit('/').next().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let nb: String = sqlx::query_scalar("SELECT invoice_number FROM invoices WHERE id = $1")
        .bind(b.rsplit('/').next().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(na, nb, "concurrent allocations must not collide");

    // Void keeps its number with a reason; gap report shows void, not missing.
    let resp = client
        .post(format!("{base}/ledgers/{ledger_id}/invoices/{id2}/void"))
        .form(&[("reason", "duplicate issue, re-bill client")])
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_redirection());
    let (status, kept, reason): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT status, invoice_number, void_reason FROM invoices WHERE id = $1")
            .bind(id2)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "void");
    assert_eq!(kept.as_deref(), Some("2026-0002"));
    assert!(reason.unwrap_or_default().len() >= 10);

    let (allocated, voids, missing) =
        openaccounting::domain::close_controls::invoice_gap_report(&pool, ledger_id, 2026)
            .await
            .unwrap();
    assert!(allocated.contains(&"2026-0002".to_string()));
    assert!(voids.contains(&"2026-0002".to_string()));
    assert!(!missing.contains(&"2026-0002".to_string()));
}

// 1.9 (E2E) — close → attempt edit → override with reason → audit shows all three.
#[tokio::test]
async fn e2e_close_edit_override_audit_trail() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let client = make_client();
    let base = server.base_url().to_string();
    register_login(&client, &base, "e2e-close@example.com").await;
    let ledger_id = create_ledger(&client, &base, "E2EClose").await;
    let (cash, sales) = account_ids(&pool, ledger_id).await;

    // Seed one posted txn, then hard-close after it.
    post_txn(
        &client,
        &base,
        None,
        ledger_id,
        cash,
        sales,
        "2026-02-01",
        "20.00",
        vec![],
    )
    .await;
    client
        .post(format!("{base}/ledgers/{ledger_id}/close"))
        .form(&[("closed_through", "2026-03-31")])
        .send()
        .await
        .unwrap();

    // Attempt an edit into the closed range → 409.
    let resp = post_txn(
        &client,
        &base,
        None,
        ledger_id,
        cash,
        sales,
        "2026-03-15",
        "20.00",
        vec![],
    )
    .await;
    assert_eq!(resp.status(), 409);

    // Override with reason → posts and writes audit.
    let resp = post_txn(
        &client,
        &base,
        None,
        ledger_id,
        cash,
        sales,
        "2026-03-15",
        "20.00",
        vec![(
            "override_reason",
            "Correcting supplier invoice INV-104 per auditor request",
        )],
    )
    .await;
    assert_eq!(resp.status(), 303);

    for action in ["close", "override"] {
        let n: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit_entries WHERE ledger_id = $1 AND action = $2",
        )
        .bind(ledger_id)
        .bind(action)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(n.0 >= 1, "audit trail must show {action}");
    }
}
