//! Integration tests for the centralized posting write path
//! (`a3-posting-service`).
//!
//! These tests cover the service directly rather than the
//! handler, so we can assert the invariants without going
//! through the form-parsing layer.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use openaccounting::domain::{
    posting_service::{NewTransaction, PostingService, PostingServiceError},
    TxnLineInput,
};
use rust_decimal::Decimal;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn bootstrap(server: &TestServer) -> (Uuid, Uuid, Uuid, Uuid) {
    // Fresh reqwest client so its cookie jar is independent of
    // the server's shared client.
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
            ("email", "posting-service@example.com"),
            ("username", "ps-user"),
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
                ("email", "posting-service@example.com"),
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

    let pool = server.db().pool();
    let (uid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("posting-service@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "PS Co"),
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

#[tokio::test]
async fn posting_service_balanced_writes_succeed() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, sales) = bootstrap(&server).await;

    let new = NewTransaction {
        ledger_id,
        txn_date: NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
        description: "Test sale".into(),
        payee: None,
        reference: None,
        kind: None,
        number: None,
        created_by: uid,
        reverses_id: None,
        lines: vec![
            TxnLineInput {
                account_id: cash,
                signed_amount: Decimal::new(100, 0),
                memo: None,
            },
            TxnLineInput {
                account_id: sales,
                signed_amount: Decimal::new(-100, 0),
                memo: None,
            },
        ],
    };
    let created = PostingService::create(&server.db().pool(), new)
        .await
        .expect("balanced transaction must succeed");
    assert_eq!(created.kind, "standard");

    // The transaction row + postings must be in the DB.
    let pool = server.db().pool();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM postings WHERE transaction_id = $1")
        .bind(created.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2, "two postings must exist");
}

#[tokio::test]
async fn posting_service_unbalanced_rejected_no_rows() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, sales) = bootstrap(&server).await;

    let new = NewTransaction {
        ledger_id,
        txn_date: NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
        description: "Off-by-one".into(),
        payee: None,
        reference: None,
        kind: None,
        number: None,
        created_by: uid,
        reverses_id: None,
        lines: vec![
            TxnLineInput {
                account_id: cash,
                signed_amount: Decimal::new(100, 0),
                memo: None,
            },
            TxnLineInput {
                account_id: sales,
                signed_amount: Decimal::new(-99, 0),
                memo: None,
            },
        ],
    };
    let err = PostingService::create(&server.db().pool(), new)
        .await
        .expect_err("unbalanced must be rejected");
    match err {
        PostingServiceError::Unbalanced { debits, credits } => {
            assert_eq!(debits, Decimal::new(100, 0));
            assert_eq!(credits, Decimal::new(99, 0));
        }
        e => panic!("expected Unbalanced, got {e:?}"),
    }

    // No transaction row should have been inserted.
    let pool = server.db().pool();
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count.0, 0, "no transactions must have been inserted");
}

#[tokio::test]
async fn posting_service_closed_period_rejected() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Close period 2025.
    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by)
         VALUES ($1, 2025, $2)",
    )
    .bind(ledger_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();

    let new = NewTransaction {
        ledger_id,
        txn_date: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        description: "Post to closed year".into(),
        payee: None,
        reference: None,
        kind: None,
        number: None,
        created_by: uid,
        reverses_id: None,
        lines: vec![
            TxnLineInput {
                account_id: cash,
                signed_amount: Decimal::new(50, 0),
                memo: None,
            },
            TxnLineInput {
                account_id: sales,
                signed_amount: Decimal::new(-50, 0),
                memo: None,
            },
        ],
    };
    let err = PostingService::create(&server.db().pool(), new)
        .await
        .expect_err("closed period must be rejected");
    assert!(
        matches!(err, PostingServiceError::PeriodClosed { year: 2025, .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn posting_service_unknown_account_rejected() {
    let server = TestServer::new().await;
    let (uid, ledger_id, cash, _sales) = bootstrap(&server).await;
    let bogus = Uuid::new_v4();

    let new = NewTransaction {
        ledger_id,
        txn_date: NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
        description: "bad acct".into(),
        payee: None,
        reference: None,
        kind: None,
        number: None,
        created_by: uid,
        reverses_id: None,
        lines: vec![
            TxnLineInput {
                account_id: cash,
                signed_amount: Decimal::new(10, 0),
                memo: None,
            },
            TxnLineInput {
                account_id: bogus,
                signed_amount: Decimal::new(-10, 0),
                memo: None,
            },
        ],
    };
    let err = PostingService::create(&server.db().pool(), new)
        .await
        .expect_err("unknown account must be rejected");
    assert!(
        matches!(err, PostingServiceError::UnknownAccount(_)),
        "got {err:?}"
    );
}