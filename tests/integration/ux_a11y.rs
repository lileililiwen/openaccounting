//! UX / a11y integration tests (`u13-ux-a11y-mobile`).
//!
//! Covers:
//!
//! * A1 — keyboard-only post of a balanced transaction moves
//!   focus to the confirmation heading and announces the total.
//! * A1 — the `a11y.js` helper bundle is served and exposes the
//!   `oaA11y` global.
//! * A1 — HTMX partials honour the `data-htmx-focus` and
//!   `data-htmx-announce` markers when present (via the show
//!   page after a posted redirect).
//! * A1 — a visually-hidden aria-live region is created on
//!   demand by the JS handler.
//! * A3 — the locale coverage script fails when a day-1 locale
//!   exceeds the threshold; passes otherwise.
//! * A4 — the mobile promise check fails when the README
//!   promises diverge and passes when they agree.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;

use crate::common::*;
use openaccounting::i18n::coverage::coverage;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_python(script: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new("python3")
        .arg(repo_root().join("scripts").join(script))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("python3 must be available");
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), combined)
}

// ── A1: focus + announce after posting ─────────────────────────────

#[tokio::test]
async fn a1_focus_and_announce_after_post() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_a11y",
            "alice_a11y@example.com",
            "correct horse battery staple",
        )
        .await;

    // Create a ledger via the public endpoint so the show
    // route has data to render.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[
            ("name", "A11y Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("ledger create");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .expect("Location");
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    // Pick two account ids to post against.
    let pool = server.db().pool();
    let (debit_id, credit_id): (Uuid, Uuid) = {
        let row: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, type FROM accounts WHERE ledger_id = $1 ORDER BY type LIMIT 2",
        )
        .bind(ledger_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(row.len() >= 2, "ledger must seed two accounts");
        (row[0].0, row[1].0)
    };

    // Submit a balanced transaction.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/transactions/new",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[
            ("date", "2026-09-21"),
            ("description", "A11y test"),
            ("lines[0][account_id]", debit_id.to_string().as_str()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100.00"),
            ("lines[1][account_id]", credit_id.to_string().as_str()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "100.00"),
            ("action", "save"),
        ])
        .send()
        .await
        .expect("POST transaction");
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "post must redirect; got {}",
        resp.status()
    );
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .expect("redirect Location");

    // The redirect URL must carry ?posted=1.
    assert!(
        loc.contains("posted=1"),
        "post-redirect URL must carry ?posted=1; got {loc}"
    );

    // Follow the redirect to the show page.
    let show_url = if loc.starts_with("http") {
        loc.clone()
    } else {
        format!("{}{}", server.base_url(), loc)
    };
    let resp = server
        .client()
        .get(show_url)
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("GET show");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("data-htmx-focus=\"#txn-confirm-heading\""),
        "show page must carry the focus marker; first 400 chars: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        body.contains("data-htmx-announce="),
        "show page must carry the announce marker"
    );
    assert!(
        body.contains("Transaction posted"),
        "announcement must name the action"
    );
    assert!(
        body.contains("txn-confirm-heading") && body.contains("tabindex=\"-1\""),
        "confirmation heading must be focusable (tabindex=-1)"
    );
    assert!(
        body.contains("/static/js/a11y.js"),
        "base layout must include the a11y helper"
    );
}

#[tokio::test]
async fn a1_a11y_js_is_served_and_exposes_helper() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/static/js/a11y.js", server.base_url()))
        .send()
        .await
        .expect("GET a11y.js");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("oaA11y"), "a11y.js must expose window.oaA11y");
    assert!(
        body.contains("htmx:afterSwap"),
        "a11y.js must listen for htmx:afterSwap"
    );
    assert!(
        body.contains("aria-live"),
        "a11y.js must use an aria-live region"
    );
}

// ── A3: locale coverage gate ───────────────────────────────────────

#[test]
fn a3_locale_coverage_gate_passes_on_real_tree() {
    let (ok, out) = run_python("check_locale_coverage.py", &[]);
    assert!(ok, "real locales must pass coverage; got:\n{out}");
}

#[test]
fn a3_locale_coverage_reporter_flags_missing_keys() {
    // Build in-memory catalogs directly to verify the pure
    // function: 1 of 3 keys missing → ~33 %.
    let mut reference = std::collections::HashMap::new();
    reference.insert("a".into(), "A".into());
    reference.insert("b".into(), "B".into());
    reference.insert("c".into(), "C".into());
    let mut de = std::collections::HashMap::new();
    de.insert("a".into(), "Ä".into());
    de.insert("c".into(), "C".into());
    let mut others = std::collections::HashMap::new();
    others.insert("de".into(), de);

    let report = coverage(&reference, &others);
    let de = report.locales.get("de").unwrap();
    assert_eq!(de.missing_keys, vec!["b".to_string()]);
    assert!(de.missing_pct > 30.0 && de.missing_pct < 34.0);
}

#[test]
fn a3_locale_coverage_gate_fails_above_threshold() {
    // Write a fixture where German is missing 8 % of keys; the
    // gate must fail and name the missing keys.
    let dir = tempfile::tempdir().unwrap();
    let locales = dir.path().join("locales");
    std::fs::create_dir_all(&locales).unwrap();

    let reference = serde_json::json!({
        "common.save": "Save",
        "common.cancel": "Cancel",
        "common.new": "New",
        "common.edit": "Edit",
        "common.delete": "Delete",
        "nav.dashboard": "Dashboard",
        "nav.reports": "Reports",
        "nav.transactions": "Transactions",
        "nav.accounts": "Accounts",
        "nav.documents": "Documents",
        "txn.one": "1 transaction",
        "txn.other": "{n} transactions",
    });
    std::fs::write(
        locales.join("en.json"),
        serde_json::to_string(&reference).unwrap(),
    )
    .unwrap();

    // 11/12 keys present → 1 missing = 8.33 %.
    let mut de = serde_json::Map::new();
    for (k, v) in reference.as_object().unwrap() {
        if k.as_str() == "common.cancel" {
            continue;
        }
        de.insert(k.clone(), v.clone());
    }
    let de_value = serde_json::Value::Object(de);
    std::fs::write(
        locales.join("de.json"),
        serde_json::to_string(&de_value).unwrap(),
    )
    .unwrap();

    let (ok, out) = run_python(
        "check_locale_coverage.py",
        &[
            format!("--locales-dir={}", locales.display()).as_str(),
            format!("--output={}", dir.path().join("coverage.json").display()).as_str(),
            "--write-output",
        ],
    );
    assert!(!ok, "gate must fail above 5 %; got:\n{out}");
    assert!(
        out.contains("de") && out.contains("8.33%"),
        "failure must name the locale and percentage; got:\n{out}"
    );
}

// ── A4: mobile promise agreement ───────────────────────────────────

#[test]
fn a4_mobile_promise_passes_on_real_checkout() {
    let (ok, out) = run_python("check_mobile_promise.py", &[]);
    assert!(
        ok,
        "real checkout must agree on the mobile promise; got:\n{out}"
    );
}

#[test]
fn a4_mobile_promise_fails_on_diverged_readme() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "# Fixture\n\n## Non-Goals\n- Native mobile apps — no commitment yet.\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("mobile")).unwrap();
    std::fs::write(
        dir.path().join("mobile").join("README.md"),
        "# Retired\n\n> **Status (2026-09-21):** Retired.\n",
    )
    .unwrap();
    let (ok, out) = run_python(
        "check_mobile_promise.py",
        &[format!("--root={}", dir.path().display()).as_str()],
    );
    assert!(!ok, "diverged promise must fail; got:\n{out}");
    assert!(
        out.contains("PWA"),
        "failure must reference the shared promise phrase; got:\n{out}"
    );
}
