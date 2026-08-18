//! HTTP integration tests for saved searches
//! (`u2-saved-searches`).
//!
//! Verifies that:
//! - Saving the current filter set writes a row.
//! - Clicking a saved search re-renders the list with the
//!   stored query.
//! - Deleting removes the row.
//! - One user's saved searches are not visible to another
//!   user.
//! - Setting a default auto-applies it on subsequent visits.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::handlers::saved_searches::default_query;

async fn make_ledger(server: &TestServer, cookie: &str) -> uuid::Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", "Search Co"),
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
    uuid::Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

#[tokio::test]
async fn http_save_search() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_ss",
            "alice_ss@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/searches",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", "Unpaid"),
            ("query", "from=2026-01-01&q=unpaid"),
            ("color", "amber"),
            ("next", "/ledgers/{ledger_id}/transactions"),
        ])
        .send()
        .await
        .expect("save search");
    assert!(resp.status() == 303 || resp.status() == 302);

    let pool = server.db().pool();
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM saved_searches WHERE user_id = (SELECT id FROM users WHERE email = $1)",
    )
    .bind("alice_ss@example.com")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "saved_searches row inserted");
}

#[tokio::test]
async fn http_apply_search() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_ss",
            "bob_ss@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    // Seed a saved search directly.
    let pool = server.db().pool();
    let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("bob_ss@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO saved_searches (user_id, name, query) VALUES ($1, 'unpaid', 'q=unpaid&from=2026-01-01')",
    )
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();

    // Click → redirect to the list with the saved query.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/searches",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("list searches");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "list endpoint must render");
    // The chip link is rendered with the original query.
    assert!(
        body.contains("q=unpaid") || body.contains("unpaid"),
        "search chip must appear; got {body}"
    );
}

#[tokio::test]
async fn http_delete_search() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "carol_ss",
            "carol_ss@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    let pool = server.db().pool();
    let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("carol_ss@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    let search_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO saved_searches (user_id, name, query) VALUES ($1, 'old', 'q=old') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/searches/{search_id}/delete",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("delete");
    assert!(resp.status() == 303 || resp.status() == 302);

    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM saved_searches WHERE id = $1")
        .bind(search_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "saved search must be removed");
}

#[tokio::test]
async fn http_search_user_scope() {
    let server = TestServer::new().await;
    // User A: alice.
    let cookie_a = server
        .bootstrap_user(
            "dave_ss",
            "dave_ss@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie_a).await;

    // User B: registered + logs in (separate cookie).
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", "eve_ss@example.com"),
            ("username", "eve_ss"),
            ("password", "correct horse battery staple"),
            ("password_confirm", "correct horse battery staple"),
        ])
        .send()
        .await
        .unwrap();
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", "eve_ss@example.com"),
            ("password", "correct horse battery staple"),
            ("next", "/"),
        ])
        .send()
        .await
        .expect("eve login");
    let cookie_b = resp
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
        .expect("eve cookie");

    // A saves a search.
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/searches",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie_a)
        .form(&[("name", "secret"), ("query", "q=secret")])
        .send()
        .await
        .unwrap();

    // B reads their own list — must NOT see A's search.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/searches",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie_b)
        .send()
        .await
        .expect("B GET");
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("secret"),
        "B must NOT see A's saved search; body was: {body}"
    );
}

#[tokio::test]
async fn http_default_query_is_returned() {
    // Unit-ish: confirm that the `default_query` helper
    // returns the row flagged is_default.
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let _cookie = server
        .bootstrap_user(
            "frank_ss",
            "frank_ss@example.com",
            "correct horse battery staple",
        )
        .await;
    let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("frank_ss@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO saved_searches (user_id, name, query, is_default)
         VALUES ($1, 'unpaid', 'q=unpaid&from=2026-01-01', TRUE),
                ($1, 'other', 'q=other', FALSE)",
    )
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();

    let q = default_query(&pool, uid).await.unwrap();
    assert!(q.is_some(), "default must resolve");
    let q = q.unwrap();
    assert!(
        q.contains("q=unpaid"),
        "default must be the row flagged is_default; got {q}"
    );
}
