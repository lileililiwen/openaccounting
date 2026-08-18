//! HTTP integration tests for localization (`u7-localization`).
//!
//! Verifies:
//! - A fresh user has `locale = 'en'` (the column default).
//! - POSTing to `/account/locale` flips the persisted value
//!   and rejects unknown values.
//! - The German locale renders dates as `31.12.2025` and
//!   EUR as `1.234,56 €`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::i18n::{fmt_currency, fmt_date, pick_locale, Locale};

#[tokio::test]
async fn http_locale_default_is_english() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_locale",
            "alice_locale@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (locale,): (String,) = sqlx::query_as("SELECT locale FROM users WHERE email = $1")
        .bind("alice_locale@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(locale, "en", "fresh user must default to English");
    let _ = cookie; // suppress unused
}

#[tokio::test]
async fn http_locale_override_saved() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_locale",
            "bob_locale@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();

    // Switch to Spanish.
    let resp = server
        .client()
        .post(format!("{}/account/locale", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("locale", "es"), ("next", "/account")])
        .send()
        .await
        .expect("set es");
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "must redirect; got {}",
        resp.status()
    );
    let (locale,): (String,) = sqlx::query_as("SELECT locale FROM users WHERE email = $1")
        .bind("bob_locale@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(locale, "es");

    // Switch to German.
    let resp = server
        .client()
        .post(format!("{}/account/locale", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("locale", "de"), ("next", "/account")])
        .send()
        .await
        .expect("set de");
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "second toggle must redirect"
    );
    let (locale,): (String,) = sqlx::query_as("SELECT locale FROM users WHERE email = $1")
        .bind("bob_locale@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(locale, "de");

    // Reject an unknown locale — must not mutate the row.
    let resp = server
        .client()
        .post(format!("{}/account/locale", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[("locale", "klingon"), ("next", "/account")])
        .send()
        .await
        .expect("set klingon");
    let (locale,): (String,) = sqlx::query_as("SELECT locale FROM users WHERE email = $1")
        .bind("bob_locale@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(locale, "de", "invalid value must NOT mutate the row");
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "invalid locale must 400/422; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn http_german_date_format() {
    // The handler is independent of the templates — call the
    // formatters directly and assert the spec's documented
    // output: `31.12.2025` and `1.234,56 €`.
    let d = chrono::NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    assert_eq!(fmt_date(d, Locale::De), "31.12.2025");
    let amt = rust_decimal_macros::dec!(1234.56);
    assert_eq!(fmt_currency(amt, "EUR", Locale::De), "1.234,56 €");

    // And that the picker actually wires the override through.
    let server = TestServer::new().await;
    server
        .bootstrap_user(
            "carol_locale",
            "carol_locale@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    sqlx::query("UPDATE users SET locale = 'de' WHERE email = $1")
        .bind("carol_locale@example.com")
        .execute(&pool)
        .await
        .unwrap();
    let (locale,): (String,) = sqlx::query_as("SELECT locale FROM users WHERE email = $1")
        .bind("carol_locale@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(locale, "de");
    assert_eq!(
        pick_locale(Some(&locale), Some("zh-CN")),
        Locale::De,
        "user override wins over Accept-Language"
    );
}
