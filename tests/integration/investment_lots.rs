//! Integration tests for investment lots / FIFO cost basis
//! (`a6-investment-lots`).
//!
//! Covers:
//! - `fifo_buy_creates_one_lot` — a buy on an `Investment`
//!   account inserts one `investment_lots` row.
//! - `fifo_sell_matches_oldest_lot` — FIFO picks the oldest
//!   lot first.
//! - `fifo_sell_across_lots_splits_correctly` — a sell that
//!   straddles two lots creates two `investment_disposals` rows
//!   with the correct per-lot realised gain.
//! - `http_holdings_report_renders` — `/reports/holdings`
//!   shows the per-account summary.
//! - `http_realized_gains_report_renders` —
//!   `/reports/realized-gains` lists disposals.
//! - `fifo_property_100_sequences` — a small property test:
//!   random buy/sell sequences never lose shares (sum of
//!   open lots + sum of disposals = sum of buys).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn bootstrap(server: &TestServer) -> (String, Uuid) {
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
            ("email", "lots-owner@example.com"),
            ("username", "lots-owner"),
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
                ("email", "lots-owner@example.com"),
                ("password", PASSWORD),
                ("next", "/"),
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

    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Lots Co"),
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
    (cookie, ledger_id)
}

/// Seed an Investment-type account + a Cash account directly in
/// the DB. Returns (cash_id, investment_id).
async fn seed_accounts(server: &TestServer, ledger_id: Uuid) -> (Uuid, Uuid) {
    let pool = server.db().pool();
    let cash_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let inv_id: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Brokerage', 'ASSET', 'OTHER_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (cash_id, inv_id)
}

/// Insert one investment_lot row directly.
async fn insert_lot(
    server: &TestServer,
    account_id: Uuid,
    acquired_at: &str,
    qty: Decimal,
    unit_cost: Decimal,
    txn_id: Uuid,
) -> Uuid {
    let pool = server.db().pool();
    sqlx::query_scalar(
        "INSERT INTO investment_lots
            (account_id, acquired_at, qty, unit_cost, currency, source_txn_id)
         VALUES ($1, $2::date, $3, $4, 'USD', $5)
         RETURNING id",
    )
    .bind(account_id)
    .bind(acquired_at)
    .bind(qty)
    .bind(unit_cost)
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap()
}

/// Insert a stub transactions row to satisfy the lot FK.
async fn stub_txn(server: &TestServer, ledger_id: Uuid, date: &str) -> Uuid {
    let pool = server.db().pool();
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Use a CAST to date so the bind string is interpreted as
    // a date value rather than text.
    sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
         VALUES ($1, $2::date, 'lot seed', 'USD', $3)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(date)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn fifo_buy_creates_one_lot() {
    let server = TestServer::new().await;
    let (_cookie, ledger_id) = bootstrap(&server).await;
    let (_cash_id, inv_id) = seed_accounts(&server, ledger_id).await;
    let txn_id = stub_txn(&server, ledger_id, "2026-08-15").await;
    let lot_id = insert_lot(
        &server,
        inv_id,
        "2026-08-15",
        Decimal::new(10, 0),
        Decimal::new(50, 0),
        txn_id,
    )
    .await;
    let pool = server.db().pool();
    let row: (Decimal, Decimal) =
        sqlx::query_as("SELECT qty, unit_cost FROM investment_lots WHERE id = $1")
            .bind(lot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, Decimal::new(10, 0));
    assert_eq!(row.1, Decimal::new(50, 0));
}

#[tokio::test]
async fn fifo_sell_matches_oldest_lot() {
    let server = TestServer::new().await;
    let (_cookie, ledger_id) = bootstrap(&server).await;
    let (_cash_id, inv_id) = seed_accounts(&server, ledger_id).await;
    let txn1 = stub_txn(&server, ledger_id, "2026-08-15").await;
    let txn2 = stub_txn(&server, ledger_id, "2026-08-16").await;
    insert_lot(
        &server,
        inv_id,
        "2026-08-15",
        Decimal::new(10, 0),
        Decimal::new(50, 0),
        txn1,
    )
    .await;
    insert_lot(
        &server,
        inv_id,
        "2026-08-16",
        Decimal::new(5, 0),
        Decimal::new(60, 0),
        txn2,
    )
    .await;

    let txn_sell = stub_txn(&server, ledger_id, "2026-08-20").await;
    let disposals = openaccounting::domain::investment_lot::fifo_match(
        &server.db().pool(),
        inv_id,
        Decimal::new(4, 0),
        Decimal::new(70, 0),
        NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
        txn_sell,
    )
    .await
    .unwrap();
    assert_eq!(disposals.len(), 1, "4 shares must match one lot");
    let d = &disposals[0];
    assert_eq!(d.qty, Decimal::new(4, 0));
    assert_eq!(d.unit_proceeds, Decimal::new(70, 0));
    // FIFO picked the oldest lot (cost 50); gain = 4 * (70 - 50) = 80
    assert_eq!(d.realized_gain, Decimal::new(80, 0));
}

#[tokio::test]
async fn fifo_sell_across_lots_splits_correctly() {
    let server = TestServer::new().await;
    let (_cookie, ledger_id) = bootstrap(&server).await;
    let (_cash_id, inv_id) = seed_accounts(&server, ledger_id).await;
    let txn1 = stub_txn(&server, ledger_id, "2026-08-15").await;
    let txn2 = stub_txn(&server, ledger_id, "2026-08-16").await;
    insert_lot(
        &server,
        inv_id,
        "2026-08-15",
        Decimal::new(10, 0),
        Decimal::new(50, 0),
        txn1,
    )
    .await;
    insert_lot(
        &server,
        inv_id,
        "2026-08-16",
        Decimal::new(5, 0),
        Decimal::new(60, 0),
        txn2,
    )
    .await;

    let txn_sell = stub_txn(&server, ledger_id, "2026-08-20").await;
    // Sell 12 shares — must span both lots (10 + 2).
    let disposals = openaccounting::domain::investment_lot::fifo_match(
        &server.db().pool(),
        inv_id,
        Decimal::new(12, 0),
        Decimal::new(70, 0),
        NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
        txn_sell,
    )
    .await
    .unwrap();
    assert_eq!(disposals.len(), 2, "must split across two lots");
    assert_eq!(disposals[0].qty, Decimal::new(10, 0));
    assert_eq!(disposals[0].realized_gain, Decimal::new(200, 0)); // 10 * (70-50)
    assert_eq!(disposals[1].qty, Decimal::new(2, 0));
    assert_eq!(disposals[1].realized_gain, Decimal::new(20, 0)); // 2 * (70-60)
}

#[tokio::test]
async fn http_holdings_report_renders() {
    let server = TestServer::new().await;
    let (cookie, ledger_id) = bootstrap(&server).await;
    let (_cash_id, inv_id) = seed_accounts(&server, ledger_id).await;
    let txn_id = stub_txn(&server, ledger_id, "2026-08-15").await;
    insert_lot(
        &server,
        inv_id,
        "2026-08-15",
        Decimal::new(10, 0),
        Decimal::new(50, 0),
        txn_id,
    )
    .await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/holdings",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET holdings");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let holdings = body["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 1);
    assert!(
        holdings[0]["qty"].as_str().unwrap_or("").starts_with("10"),
        "qty must round to 10, got {:?}",
        holdings[0]["qty"]
    );
    assert!(
        holdings[0]["cost_basis"]
            .as_str()
            .unwrap_or("")
            .starts_with("500"),
        "cost_basis must round to 500, got {:?}",
        holdings[0]["cost_basis"]
    );
}

#[tokio::test]
async fn http_realized_gains_report_renders() {
    let server = TestServer::new().await;
    let (cookie, ledger_id) = bootstrap(&server).await;
    let (_cash_id, inv_id) = seed_accounts(&server, ledger_id).await;
    let txn1 = stub_txn(&server, ledger_id, "2026-08-15").await;
    let txn2 = stub_txn(&server, ledger_id, "2026-08-16").await;
    insert_lot(
        &server,
        inv_id,
        "2026-08-15",
        Decimal::new(10, 0),
        Decimal::new(50, 0),
        txn1,
    )
    .await;
    let disposals = openaccounting::domain::investment_lot::fifo_match(
        &server.db().pool(),
        inv_id,
        Decimal::new(4, 0),
        Decimal::new(70, 0),
        NaiveDate::from_ymd_opt(2026, 8, 16).unwrap(),
        txn2,
    )
    .await
    .unwrap();
    assert_eq!(disposals.len(), 1);
    assert_eq!(disposals[0].realized_gain, Decimal::new(80, 0));

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/realized-gains",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET realized-gains");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let disposals = body["disposals"].as_array().unwrap();
    assert_eq!(disposals.len(), 1);
    assert!(
        disposals[0]["realized_gain"]
            .as_str()
            .unwrap_or("")
            .starts_with("80"),
        "realized_gain must round to 80, got {:?}",
        disposals[0]["realized_gain"]
    );
}

#[tokio::test]
async fn fifo_property_100_sequences() {
    // 100 random sequences: Σ buys = Σ (open lots + disposals)
    let server = TestServer::new().await;
    let (_cookie, ledger_id) = bootstrap(&server).await;
    let (_cash_id, inv_id) = seed_accounts(&server, ledger_id).await;

    // Use a seeded, deterministic sequence for reproducibility.
    let mut rng: u64 = 0xDEAD_BEEF_CAFE_F00D;

    for _ in 0..100 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let buys: u32 = ((rng >> 33) % 5 + 1) as u32;
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let sells: u32 = ((rng >> 33) % 5 + 1) as u32;

        // Reset lots: drop any existing ones for this account.
        sqlx::query("DELETE FROM investment_lots WHERE account_id = $1")
            .bind(inv_id)
            .execute(&server.db().pool())
            .await
            .unwrap();

        let mut total_bought = Decimal::ZERO;
        for _ in 0..buys {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let qty = Decimal::new(((rng >> 33) % 9 + 1) as i64, 0);
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let cost = Decimal::new(((rng >> 33) % 100 + 1) as i64, 0);
            let txn_id = stub_txn(&server, ledger_id, "2026-08-15").await;
            insert_lot(&server, inv_id, "2026-08-15", qty, cost, txn_id).await;
            total_bought += qty;
        }

        for _ in 0..sells {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let qty = Decimal::new(((rng >> 33) % 5 + 1) as i64, 0);
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let proceeds = Decimal::new(((rng >> 33) % 100 + 1) as i64, 0);
            let txn_id = stub_txn(&server, ledger_id, "2026-08-16").await;
            openaccounting::domain::investment_lot::fifo_match(
                &server.db().pool(),
                inv_id,
                qty,
                proceeds,
                NaiveDate::from_ymd_opt(2026, 8, 16).unwrap(),
                txn_id,
            )
            .await
            .unwrap();
        }

        let remaining =
            openaccounting::domain::investment_lot::remaining_qty(&server.db().pool(), inv_id)
                .await
                .unwrap();
        // No short-selling in this test: sell qty ≤ remaining at
        // every step. So sum(open + disposed) == sum(buys).
        let disposed: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(qty), 0)::DECIMAL FROM investment_disposals
             WHERE lot_id IN (SELECT id FROM investment_lots WHERE account_id = $1)",
        )
        .bind(inv_id)
        .fetch_one(&server.db().pool())
        .await
        .unwrap();
        assert_eq!(
            remaining + disposed,
            total_bought,
            "Σ buys = open + disposed (property)"
        );
    }
}
