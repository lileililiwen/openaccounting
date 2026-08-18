//! Round-trip tests for the plain-text accounting (PTA) CLI
//! subcommands (`d3-plaintext-export`).
//!
//! The subcommands in `src/main.rs` are three-line wrappers around
//! the library (`src/cli.rs`), so these tests drive the same code
//! paths directly: snapshot → render → import.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::common::*;
use openaccounting::export::LedgerSnapshot;
use openaccounting::import::pta::{self, PtaFormat};

/// Small deterministic PRNG so the property test is reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo) as u64)) as i64
    }
}

async fn create_ledger(server: &TestServer, cookie: &str, name: &str) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", name),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

async fn add_account(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    name: &str,
    type_: &str,
    subtype: &str,
) -> Uuid {
    sqlx::query_as::<_, (Uuid,)>(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, $2, $3, $4, 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .bind(name)
    .bind(type_)
    .bind(subtype)
    .fetch_one(pool)
    .await
    .unwrap()
    .0
}

/// Look up an account that the HTTP-ledger default chart seeded.
async fn chart_account(pool: &sqlx::PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2")
        .bind(ledger_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
async fn add_txn(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    actor: Uuid,
    date: NaiveDate,
    desc: &str,
    payee: Option<&str>,
    debit_acct: Uuid,
    credit_acct: Uuid,
    amount: Decimal,
) {
    let (txn_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO transactions (ledger_id, txn_date, description, payee, currency, created_by)
         VALUES ($1, $2, $3, $4, 'USD', $5) RETURNING id",
    )
    .bind(ledger_id)
    .bind(date)
    .bind(desc)
    .bind(payee)
    .bind(actor)
    .fetch_one(pool)
    .await
    .unwrap();
    for (acct, dir) in [(debit_acct, "DEBIT"), (credit_acct, "CREDIT")] {
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(txn_id)
        .bind(acct)
        .bind(amount)
        .bind(dir)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Canonical per-transaction signature (date, description, payee,
/// sorted legs of account|direction|amount). Equal sets ⇒ equal
/// books, independent of row ids.
#[allow(clippy::type_complexity)]
async fn txn_signatures(pool: &sqlx::PgPool, ledger_id: Uuid) -> HashSet<String> {
    let rows: Vec<(
        Uuid,
        NaiveDate,
        String,
        Option<String>,
        String,
        String,
        Decimal,
    )> = sqlx::query_as(
        "SELECT t.id, t.txn_date, t.description, t.payee, a.name, p.direction, p.amount
             FROM transactions t
             JOIN postings p ON p.transaction_id = t.id
             JOIN accounts a ON a.id = p.account_id
             WHERE t.ledger_id = $1
             ORDER BY a.name, p.direction, p.id",
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await
    .unwrap();
    let mut groups: std::collections::BTreeMap<Uuid, (NaiveDate, String, String, Vec<String>)> =
        std::collections::BTreeMap::new();
    for (id, date, desc, payee, acct, dir, amt) in rows {
        let e = groups
            .entry(id)
            .or_insert_with(|| (date, desc.clone(), payee.unwrap_or_default(), Vec::new()));
        e.3.push(format!("{acct}|{dir}|{amt}"));
    }
    groups
        .into_iter()
        .map(|(_, (date, desc, payee, legs))| format!("{date}|{desc}|{payee}|{}", legs.join(",")))
        .collect()
}

#[tokio::test]
async fn export_beancount_bean_check_parses() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "pta_bean",
            "pta_bean@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let ledger_id = create_ledger(&server, &cookie, "Bean Check").await;
    let cash = chart_account(&pool, ledger_id, "Cash on Hand").await;
    let sales = chart_account(&pool, ledger_id, "Sales Revenue").await;
    add_txn(
        &pool,
        ledger_id,
        user_id,
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        "Coffee",
        Some("Mogador"),
        cash,
        sales,
        Decimal::new(4250, 2),
    )
    .await;

    let snapshot = LedgerSnapshot::load(&pool, ledger_id).await.unwrap();
    let text = openaccounting::export::beancount::render(&snapshot);

    // `bean-check` is not installed in CI, so assert the directives
    // the strict parser requires are all present (same approach as
    // the o1 export tests).
    assert!(text.contains("option \"title\""));
    assert!(text.contains("option \"operating_currency\" \"USD\""));
    assert!(text.contains("open Cash_on_Hand USD"));
    assert!(text.contains("2026-08-01 * \"Mogador\" \"Coffee\""));
    assert!(text.contains("Cash_on_Hand"));
    assert!(text.contains("42.5000 USD"));
    assert!(text.contains("-42.5000 USD"));
    assert!(text.ends_with('\n'), "beancount must end with a newline");
}

#[tokio::test]
async fn import_beancount_inserts_rows() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "pta_import",
            "pta_import@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let source = create_ledger(&server, &cookie, "Source").await;
    let cash = chart_account(&pool, source, "Cash on Hand").await;
    let sales = chart_account(&pool, source, "Sales Revenue").await;
    add_txn(
        &pool,
        source,
        user_id,
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        "Coffee",
        Some("Mogador"),
        cash,
        sales,
        Decimal::new(4250, 2),
    )
    .await;

    let text = openaccounting::export::beancount::render(
        &LedgerSnapshot::load(&pool, source).await.unwrap(),
    );

    // Target ledger has the same default chart accounts.
    let target = create_ledger(&server, &cookie, "Target").await;
    let report = pta::import(&pool, target, user_id, PtaFormat::Beancount, &text, false)
        .await
        .unwrap();
    assert_eq!(report.errors, Vec::<String>::new());
    assert_eq!(report.inserted, 1);
    assert_eq!(report.skipped, 0);

    // Rows inserted and reports reflect them.
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{target}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Coffee") && body.contains("Mogador"),
        "transactions page must show the imported row"
    );
}

#[tokio::test]
async fn pta_round_trip_equal_for_100_random_ledgers() {
    let server = TestServer::new().await;
    let _cookie = server
        .bootstrap_user(
            "pta_prop",
            "pta_prop@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let words = [
        "Coffee", "Lunch", "Invoice", "Refund", "Rent", "Supply", "Ticket", "Deposit",
    ];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    for seed in 1..=100u64 {
        let source_id = create_ledger_sql(&pool, user_id, seed, true).await;
        let target_id = create_ledger_sql(&pool, user_id, seed, false).await;

        // Same account names in both ledgers. Names avoid spaces,
        // underscores, dashes and colons so the exporter's mangling
        // is the identity and the importer resolves by exact match.
        let mut source_accounts = Vec::new();
        let mut target_accounts = Vec::new();
        let n_accounts = 4 + (rng.next() % 3) as usize; // 4..6
        for i in 0..n_accounts {
            let (name, type_, subtype) = match i % 4 {
                0 => (format!("RandomAsset{seed}_{i}"), "ASSET", "CURRENT_ASSET"),
                1 => (
                    format!("RandomIncome{seed}_{i}"),
                    "INCOME",
                    "OPERATING_INCOME",
                ),
                2 => (
                    format!("RandomExpense{seed}_{i}"),
                    "EXPENSE",
                    "OPERATING_EXPENSE",
                ),
                _ => (
                    format!("RandomLiability{seed}_{i}"),
                    "LIABILITY",
                    "CURRENT_LIABILITY",
                ),
            };
            source_accounts.push(add_account(&pool, source_id, &name, type_, subtype).await);
            target_accounts.push(add_account(&pool, target_id, &name, type_, subtype).await);
        }

        let n_txns = 2 + (rng.next() % 3) as usize; // 2..4
        let mut inserted_here = 0usize;
        for t in 0..n_txns {
            let day = rng.range(1, 29);
            let month = rng.range(1, 13);
            let date =
                NaiveDate::from_ymd_opt(2020 + (t as i32) % 6, month as u32, day as u32).unwrap();
            let desc = format!(
                "{} {}",
                words[(rng.next() % words.len() as u64) as usize],
                t
            );
            let payee = if rng.next().is_multiple_of(3) {
                Some(format!("Vendor {t}"))
            } else {
                None
            };
            let debit = source_accounts[(rng.next() % n_accounts as u64) as usize];
            let credit = source_accounts[(rng.next() % n_accounts as u64) as usize];
            if debit == credit {
                continue;
            }
            let amount = Decimal::new(rng.range(100, 10_000) * 100, 2);
            add_txn(
                &pool,
                source_id,
                user_id,
                date,
                &desc,
                payee.as_deref(),
                debit,
                credit,
                amount,
            )
            .await;
            inserted_here += 1;
        }
        if inserted_here == 0 {
            continue;
        }

        let text = openaccounting::export::beancount::render(
            &LedgerSnapshot::load(&pool, source_id).await.unwrap(),
        );
        let report = pta::import(
            &pool,
            target_id,
            user_id,
            PtaFormat::Beancount,
            &text,
            false,
        )
        .await
        .unwrap();
        assert!(
            report.errors.is_empty(),
            "seed {seed}: unexpected import errors: {:?}",
            report.errors
        );
        assert_eq!(
            report.inserted, inserted_here,
            "seed {seed}: every exported txn must be imported"
        );
        assert_eq!(report.skipped, 0, "seed {seed}: fresh target must not skip");

        assert_eq!(
            txn_signatures(&pool, source_id).await,
            txn_signatures(&pool, target_id).await,
            "seed {seed}: round trip changed the book"
        );
    }
}

/// Create a fresh ledger directly in SQL (fast enough for 200 of
/// them). `is_source` only affects the name so source/target are
/// distinguishable.
async fn create_ledger_sql(pool: &sqlx::PgPool, owner: Uuid, seed: u64, is_source: bool) -> Uuid {
    let name = format!("Prop {seed} {}", if is_source { "S" } else { "T" });
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO ledgers (owner_id, name, base_currency, timezone, basis)
         VALUES ($1, $2, 'USD', 'UTC', 'accrual') RETURNING id",
    )
    .bind(owner)
    .bind(&name)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn hledger_csv_round_trip() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "pta_hledger",
            "pta_hledger@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let source = create_ledger(&server, &cookie, "Hledger Source").await;
    let cash = chart_account(&pool, source, "Cash on Hand").await;
    let sales = chart_account(&pool, source, "Sales Revenue").await;
    add_txn(
        &pool,
        source,
        user_id,
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        "Coffee",
        Some("Mogador"),
        cash,
        sales,
        Decimal::new(4250, 2),
    )
    .await;

    let csv_text = openaccounting::export::hledger::render(
        &LedgerSnapshot::load(&pool, source).await.unwrap(),
    );
    assert!(
        csv_text.contains("date,description,payee"),
        "must have a header; got:\n{csv_text}"
    );
    assert!(
        csv_text.contains("42.5000") && csv_text.contains("-42.5000"),
        "wide row must carry both signed legs; got:\n{csv_text}"
    );
    assert!(
        csv_text.contains("Cash on Hand") && csv_text.contains("Sales Revenue"),
        "wide row must carry both account names; got:\n{csv_text}"
    );

    let target = create_ledger(&server, &cookie, "Hledger Target").await;
    let report = pta::import(
        &pool,
        target,
        user_id,
        PtaFormat::HledgerCsv,
        &csv_text,
        false,
    )
    .await
    .unwrap();
    assert_eq!(report.errors, Vec::<String>::new());
    assert_eq!(report.inserted, 1);

    assert_eq!(
        txn_signatures(&pool, source).await,
        txn_signatures(&pool, target).await,
        "hledger round trip must preserve the book"
    );
}

#[tokio::test]
async fn import_idempotent_skip_duplicates() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "pta_dup",
            "pta_dup@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let source = create_ledger(&server, &cookie, "Dup Source").await;
    let cash = chart_account(&pool, source, "Cash on Hand").await;
    let sales = chart_account(&pool, source, "Sales Revenue").await;
    add_txn(
        &pool,
        source,
        user_id,
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        "Coffee",
        Some("Mogador"),
        cash,
        sales,
        Decimal::new(4250, 2),
    )
    .await;

    let text = openaccounting::export::beancount::render(
        &LedgerSnapshot::load(&pool, source).await.unwrap(),
    );
    let target = create_ledger(&server, &cookie, "Dup Target").await;

    let first = pta::import(&pool, target, user_id, PtaFormat::Beancount, &text, false)
        .await
        .unwrap();
    assert_eq!(first.inserted, 1);

    let second = pta::import(&pool, target, user_id, PtaFormat::Beancount, &text, false)
        .await
        .unwrap();
    assert_eq!(second.inserted, 0, "second run must be a no-op");
    assert_eq!(second.skipped, 1, "re-import must detect the duplicate");

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "no duplicate rows after re-import");
}

#[tokio::test]
async fn import_dry_run_inserts_nothing() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "pta_dry",
            "pta_dry@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let source = create_ledger(&server, &cookie, "Dry Source").await;
    let cash = chart_account(&pool, source, "Cash on Hand").await;
    let sales = chart_account(&pool, source, "Sales Revenue").await;
    add_txn(
        &pool,
        source,
        user_id,
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        "Coffee",
        Some("Mogador"),
        cash,
        sales,
        Decimal::new(4250, 2),
    )
    .await;

    let text = openaccounting::export::beancount::render(
        &LedgerSnapshot::load(&pool, source).await.unwrap(),
    );
    let target = create_ledger(&server, &cookie, "Dry Target").await;

    let report = pta::import(&pool, target, user_id, PtaFormat::Beancount, &text, true)
        .await
        .unwrap();
    assert!(report.dry_run);
    assert_eq!(report.inserted, 0, "dry run must not insert");
    assert_eq!(report.skipped, 0);
    assert!(
        report.planned.iter().any(|l| l.starts_with("insert:")),
        "dry run must print the planned diff: {:?}",
        report.planned
    );

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "dry run must leave the target untouched");
}
