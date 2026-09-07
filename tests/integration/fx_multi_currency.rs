//! Integration tests for `multi-currency-fx`: rate store, foreign
//! postings, revaluation, and the FX gains report.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use openaccounting::domain::{
    fx,
    posting_service::{NewTransaction, PostingService},
    ForeignLeg, TxnLineInput,
};
use rust_decimal::{Decimal, RoundingStrategy};
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";
const D: fn(i32, u32, u32) -> NaiveDate = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();

/// Register a user + create a USD accrual ledger; return ids plus the
/// seeded Cash on Hand / Sales Revenue accounts.
async fn bootstrap(server: &TestServer) -> (Uuid, Uuid, Uuid, Uuid) {
    let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
    let email = format!("fx-{suffix}@example.com");
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
            ("username", format!("fx-{suffix}").as_str()),
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
            ("name", format!("FX Co {suffix}").as_str()),
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
    let sales: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (uid, ledger_id, cash, sales)
}

fn dec(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

async fn insert_rate(
    pool: &sqlx::PgPool,
    base: &str,
    quote: &str,
    rate: &str,
    date: NaiveDate,
    source: &str,
) {
    sqlx::query(
        r#"INSERT INTO fx_rates (base_currency, quote_currency, rate, rate_date, source)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(base)
    .bind(quote)
    .bind(rate.parse::<Decimal>().unwrap())
    .bind(date)
    .bind(source)
    .execute(pool)
    .await
    .unwrap();
}

// ── Rate lookup ─────────────────────────────────────────────────────────

#[tokio::test]
async fn fx_lookup_picks_latest_on_or_before() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    insert_rate(&pool, "EUR", "USD", "0.91", D(2026, 8, 20), "ecb").await;
    insert_rate(&pool, "EUR", "USD", "0.92", D(2026, 8, 24), "ecb").await;

    // Transaction dated 08-23 must use the 08-20 rate.
    let rate = fx::lookup(&pool, "EUR", "USD", D(2026, 8, 23))
        .await
        .unwrap();
    assert_eq!(rate, Decimal::new(91, 2));

    // On/after 08-24 uses the newer rate.
    let rate = fx::lookup(&pool, "EUR", "USD", D(2026, 8, 25))
        .await
        .unwrap();
    assert_eq!(rate, Decimal::new(92, 2));
}

#[tokio::test]
async fn fx_lookup_uses_inverse_pair() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    // Only EUR→USD stored; USD→EUR must invert.
    insert_rate(&pool, "EUR", "USD", "0.90", D(2026, 8, 20), "ecb").await;
    let rate = fx::lookup(&pool, "USD", "EUR", D(2026, 8, 21))
        .await
        .unwrap();
    assert_eq!(
        (rate * Decimal::new(100, 0))
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero),
        Decimal::new(111, 0),
        "1/0.90 ≈ 1.1111"
    );
}

#[tokio::test]
async fn fx_lookup_missing_pair_names_pair_and_date() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let err = fx::lookup(&pool, "JPY", "USD", D(2026, 8, 23))
        .await
        .expect_err("no rate stored");
    let msg = err.to_string();
    assert!(msg.contains("JPY"), "message names base: {msg}");
    assert!(msg.contains("USD"), "message names quote: {msg}");
    assert!(msg.contains("2026-08-23"), "message names date: {msg}");
}

// ── Foreign postings through the posting service ────────────────────────

#[tokio::test]
async fn foreign_invoice_converts_at_transaction_date_rate() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    insert_rate(&pool, "EUR", "USD", "0.91", D(2026, 8, 20), "ecb").await;

    let created = PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: D(2026, 8, 23),
            description: "EUR invoice".into(),
            payee: None,
            reference: None,
            kind: None,
            created_by: uid,
            reverses_id: None,
            number: None,
            tax_links: vec![],
            lines: vec![
                TxnLineInput {
                    account_id: cash,
                    signed_amount: Decimal::ZERO, // ignored: derived from foreign leg
                    memo: None,
                    tax_rate_id: None,
                    foreign: Some(ForeignLeg {
                        signed_amount: Decimal::new(100_000, 2), // 1000.00 EUR debit
                        currency: "EUR".into(),
                    }),
                },
                TxnLineInput {
                    account_id: sales,
                    signed_amount: -dec(91_000),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
            ],
        },
    )
    .await
    .expect("foreign transaction must succeed");

    let row: (Decimal, Option<Decimal>, Option<String>) = sqlx::query_as(
        "SELECT amount, foreign_amount, foreign_currency FROM postings
         WHERE transaction_id = $1 AND direction = 'DEBIT'",
    )
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, dec(91_000), "base amount derived at txn-date rate");
    assert_eq!(row.1, Some(Decimal::new(100_000, 2)));
    assert_eq!(row.2.as_deref(), Some("EUR"));

    // The DB balance trigger held (insert would have failed otherwise);
    // assert explicitly anyway.
    let balanced: (bool,) = sqlx::query_as(
        "SELECT (
            SELECT COALESCE(SUM(CASE WHEN direction='DEBIT' THEN amount ELSE -amount END), 0)
            FROM postings WHERE transaction_id = $1
        ) = 0",
    )
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(balanced.0);
}

#[tokio::test]
async fn missing_rate_blocks_posting_without_rows() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    let err = PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: D(2026, 8, 23),
            description: "JPY expense".into(),
            payee: None,
            reference: None,
            kind: None,
            created_by: uid,
            reverses_id: None,
            number: None,
            tax_links: vec![],
            lines: vec![
                TxnLineInput {
                    account_id: cash,
                    signed_amount: Decimal::ZERO,
                    memo: None,
                    tax_rate_id: None,
                    foreign: Some(ForeignLeg {
                        signed_amount: Decimal::new(10_000, 0),
                        currency: "JPY".into(),
                    }),
                },
                TxnLineInput {
                    account_id: sales,
                    signed_amount: -dec(1_000),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
            ],
        },
    )
    .await
    .expect_err("missing JPY→USD rate must block");

    let msg = err.to_string();
    assert!(msg.contains("JPY") && msg.contains("USD"), "{msg}");
    assert!(msg.contains("2026-08-23"), "{msg}");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "nothing written when a rate is missing");
}

/// Property-style: random foreign legs always produce base-balanced
/// transactions (the DB trigger is the backstop; we assert in SQL too).
#[tokio::test]
async fn property_random_foreign_postings_stay_balanced() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    insert_rate(&pool, "EUR", "USD", "0.91", D(2026, 1, 1), "ecb").await;

    let mut seed = 0x5eed_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    for _ in 0..50 {
        let eur_cents = (next() % 500_000) as i64 + 100; // 1.00 … 5000.99
        let foreign = Decimal::new(eur_cents, 2);
        let base = fx::derive_base_amount(foreign, Decimal::new(91, 2));

        let created = PostingService::create(
            &pool,
            NewTransaction {
                ledger_id,
                txn_date: D(2026, 2, 15),
                description: "property case".into(),
                payee: None,
                reference: None,
                kind: None,
                created_by: uid,
                reverses_id: None,
                number: None,
                tax_links: vec![],
                lines: vec![
                    TxnLineInput {
                        account_id: cash,
                        signed_amount: Decimal::ZERO,
                        memo: None,
                        tax_rate_id: None,
                        foreign: Some(ForeignLeg {
                            signed_amount: foreign,
                            currency: "EUR".into(),
                        }),
                    },
                    TxnLineInput {
                        account_id: sales,
                        signed_amount: -base,
                        memo: None,
                        tax_rate_id: None,
                        foreign: None,
                    },
                ],
            },
        )
        .await
        .expect("random case must succeed");

        let net: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(CASE WHEN direction='DEBIT' THEN amount ELSE -amount END), 0)
             FROM postings WHERE transaction_id = $1",
        )
        .bind(created.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(net.0, Decimal::ZERO, "case with foreign={foreign}");
    }
}

// ── Manual overrides feed ───────────────────────────────────────────────

#[tokio::test]
async fn manual_rate_overrides_feed_for_same_day() {
    let server = TestServer::new().await;
    let (_uid, ledger_id, _cash, _sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Feed stores its rate first.
    insert_rate(&pool, "EUR", "USD", "1.08", D(2026, 8, 21), "ecb").await;

    // Owner enters a manual override via the HTTP form.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/fx-rates",
            server.base_url()
        ))
        .form(&[
            ("base_currency", "EUR"),
            ("quote_currency", "USD"),
            ("rate", "1.09"),
            ("rate_date", "2026-08-21"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let (rate, source): (Decimal, String) = sqlx::query_as(
        "SELECT rate, source FROM fx_rates
         WHERE base_currency='EUR' AND quote_currency='USD' AND rate_date=$1",
    )
    .bind(D(2026, 8, 21))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rate, Decimal::new(109, 2));
    assert_eq!(source, "manual");

    // Exactly one row for the pair/day (UNIQUE holds).
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM fx_rates
         WHERE base_currency='EUR' AND quote_currency='USD' AND rate_date=$1",
    )
    .bind(D(2026, 8, 21))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 1);
}

// ── Revaluation ─────────────────────────────────────────────────────────

/// Create an EUR-denominated bank account holding 1000 EUR whose book
/// (base) value is `book_usd_cents`, via two foreign postings.
async fn seed_euro_account(
    server: &TestServer,
    ledger_id: Uuid,
    uid: Uuid,
    book_usd_cents: i64,
) -> Uuid {
    let pool = server.db().pool();
    let eur_account: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Euro Bank', 'ASSET', 'CURRENT_ASSET', 'EUR') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    insert_rate(&pool, "EUR", "USD", "0.91", D(2026, 1, 1), "ecb").await;
    PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: D(2026, 1, 2),
            description: "open EUR account".into(),
            payee: None,
            reference: None,
            kind: None,
            created_by: uid,
            reverses_id: None,
            number: None,
            tax_links: vec![],
            lines: vec![
                TxnLineInput {
                    account_id: eur_account,
                    signed_amount: Decimal::ZERO,
                    memo: None,
                    tax_rate_id: None,
                    foreign: Some(ForeignLeg {
                        signed_amount: Decimal::new(100_000, 2),
                        currency: "EUR".into(),
                    }),
                },
                TxnLineInput {
                    account_id: seed_equity(server, ledger_id).await,
                    signed_amount: -dec(book_usd_cents),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    eur_account
}

async fn seed_equity(server: &TestServer, ledger_id: Uuid) -> Uuid {
    let pool = server.db().pool();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Owner Equity FX', 'EQUITY', 'EQUITY', 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn revaluation_posts_balanced_loss_once_per_month() {
    let server = TestServer::new().await;
    let (uid, ledger_id, _cash, _sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    // 1000 EUR booked at 0.91 → 910 USD book value.
    let eur_account = seed_euro_account(&server, ledger_id, uid, 91_000).await;

    // Month-end rate 0.89 → value 890 USD → 20 USD unrealized loss.
    insert_rate(&pool, "EUR", "USD", "0.89", D(2026, 1, 31), "ecb").await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/fx-revaluation",
            server.base_url()
        ))
        .form(&[("date", "2026-01-31")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    // One revaluation row with -20.00.
    let (fx_gain_loss, txn_id): (Decimal, Uuid) = sqlx::query_as(
        "SELECT fx_gain_loss, transaction_id FROM fx_revaluations
         WHERE ledger_id = $1 AND account_id = $2",
    )
    .bind(ledger_id)
    .bind(eur_account)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(fx_gain_loss, dec(-2_000));

    // The posted transaction balances.
    let net: (Decimal,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN direction='DEBIT' THEN amount ELSE -amount END), 0)
         FROM postings WHERE transaction_id = $1",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(net.0, Decimal::ZERO);

    // Second run for the same month → 409 Conflict.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/fx-revaluation",
            server.base_url()
        ))
        .form(&[("date", "2026-01-31")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);

    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM fx_revaluations WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n.0, 1, "no duplicate revaluation posted");
}

// ── FX gains report ─────────────────────────────────────────────────────

#[tokio::test]
async fn fx_gains_report_separates_realized_from_unrealized() {
    let server = TestServer::new().await;
    let (uid, ledger_id, _cash, _sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    let eur_account = seed_euro_account(&server, ledger_id, uid, 91_000).await;
    insert_rate(&pool, "EUR", "USD", "0.89", D(2026, 1, 31), "ecb").await;

    // Post the January revaluation (unrealized section).
    fx::run_revaluation(&pool, ledger_id, D(2026, 1, 31), uid)
        .await
        .unwrap();

    // A manual adjustment hitting the FX loss account (realized section):
    // find the auto-created loss account and post against it directly.
    let (loss_account,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'FX Unrealized Loss'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: D(2026, 2, 10),
            description: "settled EUR payable at bank rate".into(),
            payee: None,
            reference: None,
            kind: None,
            created_by: uid,
            reverses_id: None,
            number: None,
            tax_links: vec![],
            lines: vec![
                TxnLineInput {
                    account_id: loss_account,
                    signed_amount: dec(500),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
                TxnLineInput {
                    account_id: eur_account,
                    signed_amount: -dec(500),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
            ],
        },
    )
    .await
    .unwrap();

    let html = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/fx-gains",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(html.status(), 200);
    let body = html.text().await.unwrap();
    assert!(body.contains("Unrealized"), "section present");
    assert!(body.contains("Realized"), "section present");
    assert!(body.contains("-20.00"), "unrealized loss figure shown");
    assert!(body.contains("5.00"), "realized figure shown");

    // CSV export works and separates sections.
    let csv = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/fx-gains/export.csv",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(csv.status(), 200);
    let csv_body = csv.text().await.unwrap();
    assert!(csv_body.contains("unrealized,"));
    assert!(csv_body.contains("realized,"));
}

// ── ECB feed idempotency (worker core, no network) ──────────────────────

#[tokio::test]
async fn ecb_upsert_is_idempotent_and_backfills_gaps() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let csv = "Date,USD,JPY\n2026-08-19,1.0850,167.10\n2026-08-20,1.0877,168.00\n2026-08-21,1.0891,168.42\n";
    let rates = fx::parse_ecb_csv(csv);
    assert_eq!(rates.len(), 6);

    let first = fx::upsert_ecb_rates(&pool, &rates).await.unwrap();
    assert_eq!(first, 6);
    // Re-running the same day backfills nothing.
    let second = fx::upsert_ecb_rates(&pool, &rates).await.unwrap();
    assert_eq!(second, 0);

    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM fx_rates WHERE source='ecb'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n.0, 6);
}
