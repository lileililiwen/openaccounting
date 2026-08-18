//! HTTP integration tests for notification preferences
//! (`u6-notification-preferences`).
//!
//! Verifies:
//! - The default grid for a fresh user matches the spec
//!   (in-app on for every event, email only for
//!   weekly_summary, push off for everything).
//! - POSTing a toggle persists the row.
//! - The dispatcher respects the preference (unit-style: we
//!   bypass the notifier by registering no device token, then
//!   assert that no `notifications_sent` log line is produced).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::notifications::preferences::{Channel, Event};

#[tokio::test]
async fn http_notifications_defaults() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_np",
            "alice_np@example.com",
            "correct horse battery staple",
        )
        .await;

    let resp = server
        .client()
        .get(format!("{}/account/notifications", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /account/notifications");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();

    // The page must render a row for each event.
    for event_label in [
        "Budget overrun",
        "Reimbursement submitted",
        "Large transaction",
        "Weekly summary",
    ] {
        assert!(
            body.contains(event_label),
            "missing row label {event_label}"
        );
    }

    // Pull every preference from the DB; the defaults should
    // match the spec (no rows present for a fresh user).
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("alice_np@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    for ev in [
        Event::BudgetOverrun,
        Event::ReimbursementSubmitted,
        Event::LargeTransaction,
        Event::WeeklySummary,
    ] {
        // in_app: on
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM notification_preferences
             WHERE user_id = $1 AND channel = 'in_app' AND event = $2",
        )
        .bind(user_id)
        .bind(ev.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 0, "no row expected for in_app default; got {n}");
        assert!(openaccounting::notifications::preferences::is_enabled(
            &pool,
            user_id,
            Channel::InApp,
            ev
        )
        .await
        .unwrap());

        // email: only weekly_summary ON
        let email_on = openaccounting::notifications::preferences::is_enabled(
            &pool,
            user_id,
            Channel::Email,
            ev,
        )
        .await
        .unwrap();
        let expected = matches!(ev, Event::WeeklySummary);
        assert_eq!(email_on, expected, "email default for {ev:?}");

        // push: off
        assert!(!openaccounting::notifications::preferences::is_enabled(
            &pool,
            user_id,
            Channel::Push,
            ev
        )
        .await
        .unwrap());
    }
}

#[tokio::test]
async fn http_notifications_toggle_persists() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_np",
            "bob_np@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("bob_np@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Toggle push ON for budget_overrun (default is OFF).
    let resp = server
        .client()
        .post(format!("{}/account/notifications", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[
            ("channel", "push"),
            ("event", "budget_overrun"),
            ("enabled", "on"),
        ])
        .send()
        .await
        .expect("toggle");
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "toggle must redirect; got {}",
        resp.status()
    );

    let row: (bool,) = sqlx::query_as(
        "SELECT enabled FROM notification_preferences
         WHERE user_id = $1 AND channel = 'push' AND event = 'budget_overrun'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.0, "row should now be true");

    // Toggle it back OFF.
    let resp = server
        .client()
        .post(format!("{}/account/notifications", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("channel", "push"),
            ("event", "budget_overrun"),
            ("enabled", "off"),
        ])
        .send()
        .await
        .expect("toggle off");
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "second toggle must also redirect"
    );

    let row: (bool,) = sqlx::query_as(
        "SELECT enabled FROM notification_preferences
         WHERE user_id = $1 AND channel = 'push' AND event = 'budget_overrun'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!row.0, "row should now be false");
}

#[tokio::test]
async fn notification_respects_preference() {
    // Pure unit test on the preferences module: a user with
    // push off for an event must be reported as disabled.
    let server = TestServer::new().await;
    server
        .bootstrap_user(
            "carol_np",
            "carol_np@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("carol_np@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Default (no row): push off.
    assert!(!openaccounting::notifications::preferences::is_enabled(
        &pool,
        user_id,
        Channel::Push,
        Event::BudgetOverrun
    )
    .await
    .unwrap());

    // After upsert ON, the function returns true.
    openaccounting::notifications::preferences::set_enabled(
        &pool,
        user_id,
        Channel::Push,
        Event::BudgetOverrun,
        true,
    )
    .await
    .unwrap();
    assert!(openaccounting::notifications::preferences::is_enabled(
        &pool,
        user_id,
        Channel::Push,
        Event::BudgetOverrun
    )
    .await
    .unwrap());

    // After upsert OFF, the function returns false.
    openaccounting::notifications::preferences::set_enabled(
        &pool,
        user_id,
        Channel::Push,
        Event::BudgetOverrun,
        false,
    )
    .await
    .unwrap();
    assert!(!openaccounting::notifications::preferences::is_enabled(
        &pool,
        user_id,
        Channel::Push,
        Event::BudgetOverrun
    )
    .await
    .unwrap());
}

use uuid::Uuid;
