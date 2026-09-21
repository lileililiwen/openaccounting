//! Integration tests for accounting dimensions
//! (`accounting-dimensions`): posting dimensions + sliced reports,
//! recurring journals (preview/idempotency/skip), inventory method
//! guard, declining-balance depreciation + disposal posting, and the
//! FX override audit. Drives the real axum router via `TestServer`.

use crate::common::*;
use chrono::NaiveDate;
use openaccounting::domain::{
    dimensions::DimensionFilter,
    posting_service::{NewTransaction, PostingService},
    TxnLineInput,
};
use openaccounting::reports::{
    build_income_statement_filtered, build_trial_balance_filtered, ReportBasis,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

async fn bootstrap(server: &TestServer, tag: &str) -> (String, Uuid, Uuid) {
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
    let email = format!("{tag}@example.com");
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("username", tag),
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
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Dimensions Co"),
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
    (cookie, uid, ledger_id)
}

async fn account_id(pool: &PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2")
        .bind(ledger_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn post_form(
    server: &TestServer,
    cookie: &str,
    url: &str,
    fields: &[(&str, &str)],
) -> reqwest::Response {
    let body = fields
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    server
        .client()
        .post(url)
        .header(reqwest::header::COOKIE, cookie)
        .header("X-OA-CSRF-Bypass", "1")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("post form")
}

async fn get_page(server: &TestServer, cookie: &str, url: &str) -> (reqwest::StatusCode, String) {
    let resp = server
        .client()
        .get(url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("get");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    (status, body)
}

async fn make_dimension(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    kind: &str,
    name: &str,
) -> Uuid {
    let pool = server.db().pool();
    let resp = post_form(
        server,
        cookie,
        &format!(
            "{}/ledgers/{}/dimensions/{}",
            server.base_url(),
            ledger_id,
            kind
        ),
        &[("name", name)],
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "create {kind} redirects, got {}",
        resp.status()
    );
    let table = if kind == "cost-centers" {
        "cost_centers"
    } else {
        "projects"
    };
    sqlx::query_scalar(&format!(
        "SELECT id FROM {table} WHERE ledger_id = $1 AND name = $2"
    ))
    .bind(ledger_id)
    .bind(name)
    .fetch_one(&pool)
    .await
    .unwrap()
}

// ─── 1.1 / 1.6: dimension slices + Unassigned ────────────────────────────────

#[tokio::test]
async fn dimension_slice_partitions_and_unassigned_holds_untagged() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, uid, ledger_id) = bootstrap(&server, "dim-user-1").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let expense = account_id(&pool, ledger_id, "Other Expense").await;

    let cc = make_dimension(&server, &cookie, ledger_id, "cost-centers", "Berlin").await;
    let p1 = make_dimension(&server, &cookie, ledger_id, "projects", "P1").await;
    let p2 = make_dimension(&server, &cookie, ledger_id, "projects", "P2").await;

    // Fund the ledger so the trial balance has two-sided rows.
    let equity = account_id(&pool, ledger_id, "Owner's Equity").await;
    PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            description: "opening".into(),
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
                    signed_amount: dec("1000.00"),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                    cost_center_id: Some(cc),
                    project_id: None,
                },
                TxnLineInput {
                    account_id: equity,
                    signed_amount: dec("-1000.00"),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                    cost_center_id: Some(cc),
                    project_id: None,
                },
            ],
        },
    )
    .await
    .expect("opening balances");

    // Two expense postings on the same account, different projects.
    // Both legs carry the cost center (dimension slicing balances
    // when the whole journal is tagged).
    for (proj, amount) in [(Some(p1), "100.00"), (Some(p2), "50.00"), (None, "25.00")] {
        PostingService::create(
            &pool,
            NewTransaction {
                ledger_id,
                txn_date: NaiveDate::from_ymd_opt(2026, 5, 10).unwrap(),
                description: "project spend".into(),
                payee: None,
                reference: None,
                kind: None,
                created_by: uid,
                reverses_id: None,
                number: None,
                tax_links: vec![],
                lines: vec![
                    TxnLineInput {
                        account_id: expense,
                        signed_amount: dec(amount),
                        memo: None,
                        tax_rate_id: None,
                        foreign: None,
                        cost_center_id: Some(cc),
                        project_id: proj,
                    },
                    TxnLineInput {
                        account_id: cash,
                        signed_amount: -dec(amount),
                        memo: None,
                        tax_rate_id: None,
                        foreign: None,
                        cost_center_id: Some(cc),
                        project_id: None,
                    },
                ],
            },
        )
        .await
        .expect("dimensioned posting keeps the balance invariant");
    }

    // P&L filtered to P1 shows only the P1 amount (numeric assertion
    // on the builder — page HTML also carries UUIDs that defeat
    // substring checks); the untagged 25.00 sits in Unassigned.
    let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
    let sliced = build_income_statement_filtered(
        &pool,
        ledger_id,
        from,
        to,
        ReportBasis::Accrual,
        &DimensionFilter {
            cost_center_id: None,
            project_id: Some(p1),
        },
    )
    .await
    .unwrap();
    assert_eq!(sliced.operating_expenses.total, dec("100.00"));
    let un = sliced.unassigned.expect("Unassigned bucket with a filter");
    assert_eq!(un.expense, dec("25.00"), "only the untagged line");
    assert_eq!(un.revenue, dec("0"));

    let full = build_income_statement_filtered(
        &pool,
        ledger_id,
        from,
        to,
        ReportBasis::Accrual,
        &DimensionFilter::empty(),
    )
    .await
    .unwrap();
    assert_eq!(full.operating_expenses.total, dec("175.00"));
    assert!(full.unassigned.is_none(), "no bucket without a filter");

    // HTTP: the filtered page renders the Unassigned bucket.
    let (status, body) = get_page(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reports/income-statement?from=2026-01-01&to=2026-12-31&project_id={}",
            server.base_url(),
            ledger_id,
            p1
        ),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains("Unassigned"), "untagged postings bucketed");

    // TB slice by cost center: every leg carries Berlin, so the slice
    // balances at the funded totals and Unassigned is empty.
    let tb = build_trial_balance_filtered(
        &pool,
        ledger_id,
        to,
        &DimensionFilter {
            cost_center_id: Some(cc),
            project_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(tb.total_debit, dec("1000.00"));
    assert_eq!(
        tb.total_debit, tb.total_credit,
        "fully-tagged slice balances"
    );
    // Debit-normal display: the 175.00 expense debit shows on its row;
    // the offsetting cash credit sits on a debit-normal account and
    // renders zero (existing TB display rule — slices need not balance
    // when the contra leg is untagged or contra-normal).
    let exp_row = tb
        .rows
        .iter()
        .find(|r| r.account_id == expense)
        .expect("expense row in slice");
    assert_eq!(exp_row.debit, dec("175.00"));
    let (ud, uc) = tb.unassigned.expect("Unassigned bucket with a filter");
    // Every leg carries Berlin, so Unassigned is empty.
    assert_eq!(ud, dec("0"));
    assert_eq!(uc, dec("0"));

    // Partition: filtered slice + Unassigned == unfiltered totals.
    let full = build_trial_balance_filtered(&pool, ledger_id, to, &DimensionFilter::empty())
        .await
        .unwrap();
    assert_eq!(full.total_debit, dec("1000.00"));
    assert_eq!(full.total_debit, full.total_credit);

    let (status, _body) = get_page(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/reports/trial-balance?as_of=2026-12-31&cost_center_id={}",
            server.base_url(),
            ledger_id,
            cc
        ),
    )
    .await;
    assert_eq!(status, 200);
}

// ─── 1.4: preview generates drafts without posting ───────────────────────────

async fn make_template(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    debit: Uuid,
    credit: Uuid,
    start: &str,
) -> Uuid {
    let pool = server.db().pool();
    let resp = post_form(
        server,
        cookie,
        &format!(
            "{}/ledgers/{}/recurring-journals",
            server.base_url(),
            ledger_id
        ),
        &[
            ("name", "Monthly rent"),
            ("frequency", "monthly"),
            ("start_date", start),
            ("lines[0][account_id]", &debit.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "1000.00"),
            ("lines[1][account_id]", &credit.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "1000.00"),
        ],
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "template created, got {}",
        resp.status()
    );
    sqlx::query_scalar(
        "SELECT id FROM recurring_journal_templates WHERE ledger_id = $1 AND name = $2",
    )
    .bind(ledger_id)
    .bind("Monthly rent")
    .fetch_one(&pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn recurring_preview_generates_draft_without_posting() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, _uid, ledger_id) = bootstrap(&server, "dim-user-2").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let expense = account_id(&pool, ledger_id, "Other Expense").await;
    let tpl = make_template(&server, &cookie, ledger_id, expense, cash, "2026-04-01").await;

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/recurring-journals/{}/preview",
            server.base_url(),
            ledger_id,
            tpl
        ),
        &[("period", "2026-04-01")],
    )
    .await;
    assert!(resp.status().is_redirection());

    let (drafts,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind = 'draft'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(drafts, 1, "preview creates exactly one draft");
    let (posted,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind != 'draft'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(posted, 0, "preview posts nothing");

    // Explicit post promotes the draft.
    let (run,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM recurring_journal_runs WHERE template_id = $1 AND period_key = '2026-04-01'",
    )
    .bind(tpl)
    .fetch_one(&pool)
    .await
    .unwrap();
    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/recurring-journals/runs/{}/post",
            server.base_url(),
            ledger_id,
            run
        ),
        &[],
    )
    .await;
    assert!(resp.status().is_redirection());
    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM recurring_journal_runs WHERE id = $1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "posted");
}

// ─── 1.7: double-fire posts once ─────────────────────────────────────────────

#[tokio::test]
async fn scheduler_double_fire_generates_one_journal_per_period() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, _uid, ledger_id) = bootstrap(&server, "dim-user-3").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let expense = account_id(&pool, ledger_id, "Other Expense").await;
    make_template(&server, &cookie, ledger_id, expense, cash, "2026-01-01").await;

    let url = format!(
        "{}/ledgers/{}/recurring-journals/run-due",
        server.base_url(),
        ledger_id
    );
    assert!(post_form(&server, &cookie, &url, &[])
        .await
        .status()
        .is_redirection());
    assert!(post_form(&server, &cookie, &url, &[])
        .await
        .status()
        .is_redirection());

    // One run per period: the 2026-01 period exists exactly once even
    // though the scheduler fired twice (plus catch-up for later months).
    let (jan,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recurring_journal_runs r
         JOIN recurring_journal_templates t ON t.id = r.template_id
         WHERE t.ledger_id = $1 AND r.period_key = '2026-01-01'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(jan, 1, "double-fire yields one January journal");
}

// ─── Skip excludes a period, later periods continue ──────────────────────────

#[tokio::test]
async fn skip_excludes_period_but_later_periods_continue() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, _uid, ledger_id) = bootstrap(&server, "dim-user-4").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let expense = account_id(&pool, ledger_id, "Other Expense").await;
    let tpl = make_template(&server, &cookie, ledger_id, expense, cash, "2026-01-01").await;

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/recurring-journals/{}/skip",
            server.base_url(),
            ledger_id,
            tpl
        ),
        &[("period_key", "2026-02-01")],
    )
    .await;
    assert!(resp.status().is_redirection());

    let url = format!(
        "{}/ledgers/{}/recurring-journals/run-due",
        server.base_url(),
        ledger_id
    );
    post_form(&server, &cookie, &url, &[]).await;

    let (skipped,): (String,) = sqlx::query_as(
        "SELECT status FROM recurring_journal_runs WHERE template_id = $1 AND period_key = '2026-02-01'",
    )
    .bind(tpl)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(skipped, "skipped");
    let (draft,): (Option<Uuid>,) = sqlx::query_as(
        "SELECT draft_txn_id FROM recurring_journal_runs WHERE template_id = $1 AND period_key = '2026-02-01'",
    )
    .bind(tpl)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(draft.is_none(), "skipped period generates no draft");

    // March still generates.
    let (mar,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recurring_journal_runs WHERE template_id = $1 AND period_key = '2026-03-01' AND status = 'preview'",
    )
    .bind(tpl)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mar, 1, "later periods continue after a skip");
}

// ─── 1.8: FX override audit ──────────────────────────────────────────────────

#[tokio::test]
async fn fx_override_without_reason_fails_with_reason_audits() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, uid, ledger_id) = bootstrap(&server, "dim-user-5").await;

    sqlx::query(
        "INSERT INTO fx_rates (base_currency, quote_currency, rate, rate_date, source)
         VALUES ('EUR', 'USD', 1.08, '2026-08-21', 'ecb')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let url = format!("{}/ledgers/{}/fx-rates", server.base_url(), ledger_id);
    let resp = post_form(
        &server,
        &cookie,
        &url,
        &[
            ("base_currency", "EUR"),
            ("quote_currency", "USD"),
            ("rate", "1.09"),
            ("rate_date", "2026-08-21"),
        ],
    )
    .await;
    assert_eq!(resp.status(), 400, "reasonless override is rejected");

    let resp = post_form(
        &server,
        &cookie,
        &url,
        &[
            ("base_currency", "EUR"),
            ("quote_currency", "USD"),
            ("rate", "1.09"),
            ("rate_date", "2026-08-21"),
            ("reason", "bank fixing rate"),
        ],
    )
    .await;
    assert!(resp.status().is_redirection(), "reasoned override applies");

    let (rate, source): (Decimal, String) = sqlx::query_as(
        "SELECT rate, source FROM fx_rates
         WHERE base_currency = 'EUR' AND quote_currency = 'USD' AND rate_date = '2026-08-21'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rate, dec("1.09"));
    assert_eq!(source, "manual");

    let row: (Uuid, Option<Decimal>, Decimal, String) = sqlx::query_as(
        "SELECT actor_id, old_rate, new_rate, reason FROM fx_override_audit
         WHERE ledger_id = $1 AND base_currency = 'EUR' AND rate_date = '2026-08-21'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, uid, "audit records the actor");
    assert_eq!(row.1, Some(dec("1.08")), "audit records the old value");
    assert_eq!(row.2, dec("1.09"), "audit records the new value");
    assert_eq!(row.3, "bank fixing rate");
}

// ─── Valuation method switch blocked with stock ──────────────────────────────

async fn make_item(pool: &PgPool, ledger_id: Uuid) -> Uuid {
    let asset = account_id(pool, ledger_id, "Cash on Hand").await;
    let cogs = account_id(pool, ledger_id, "Other Expense").await;
    let income = account_id(pool, ledger_id, "Sales Revenue").await;
    sqlx::query_scalar(
        "INSERT INTO inventory_items (ledger_id, name, asset_account_id, cogs_account_id, income_account_id)
         VALUES ($1, 'Widget', $2, $3, $4) RETURNING id",
    )
    .bind(ledger_id)
    .bind(asset)
    .bind(cogs)
    .bind(income)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn inventory_method_switch_blocked_with_stock() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, _uid, ledger_id) = bootstrap(&server, "dim-user-6").await;
    let item = make_item(&pool, ledger_id).await;

    // Purchase 10 units: stock is now nonzero.
    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/inventory/{}/purchase",
            server.base_url(),
            ledger_id,
            item
        ),
        &[("quantity", "10"), ("unit_cost", "5.00")],
    )
    .await;
    assert!(resp.status().is_redirection());

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/inventory/method",
            server.base_url(),
            ledger_id
        ),
        &[("method", "fifo")],
    )
    .await;
    assert_eq!(resp.status(), 409, "method switch with stock is rejected");
    let body = resp.text().await.expect("body");
    assert!(body.contains("10"), "409 names the blocking stock: {body}");

    // Valuation page discloses the (unchanged) method.
    let (status, body) = get_page(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/inventory/valuation",
            server.base_url(),
            ledger_id
        ),
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        body.contains("average"),
        "method disclosed on valuation: {body}"
    );
}

// ─── Declining-balance depreciation + disposal gain/loss ─────────────────────

async fn make_asset(pool: &PgPool, ledger_id: Uuid, method: &str) -> Uuid {
    let asset = account_id(pool, ledger_id, "Cash on Hand").await;
    sqlx::query_scalar(
        "INSERT INTO fixed_assets (ledger_id, name, account_id, purchase_date, purchase_cost, salvage_value, useful_life_years, depreciation_method)
         VALUES ($1, 'Press', $2, '2026-01-15', 12000, 2000, 5, $3) RETURNING id",
    )
    .bind(ledger_id)
    .bind(asset)
    .bind(method)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn declining_balance_first_charge_matches_schedule() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, _uid, ledger_id) = bootstrap(&server, "dim-user-7").await;
    let asset = make_asset(&pool, ledger_id, "declining_balance").await;

    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/fixed-assets/{}/depreciate",
            server.base_url(),
            ledger_id,
            asset
        ),
        &[],
    )
    .await;
    assert!(resp.status().is_redirection());
    let (acc,): (Decimal,) =
        sqlx::query_as("SELECT accumulated_depreciation FROM fixed_assets WHERE id = $1")
            .bind(asset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(acc, dec("400.00"), "DDB month 1 = 12000 × 40% / 12");
}

#[tokio::test]
async fn disposal_posts_gain_and_balances() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (cookie, _uid, ledger_id) = bootstrap(&server, "dim-user-8").await;
    let asset = make_asset(&pool, ledger_id, "straight_line").await;

    // NBV = 12,000; sell for 13,000 → 1,000 gain.
    let resp = post_form(
        &server,
        &cookie,
        &format!(
            "{}/ledgers/{}/fixed-assets/{}/dispose",
            server.base_url(),
            ledger_id,
            asset
        ),
        &[
            ("disposed_date", "2026-06-01"),
            ("disposed_amount", "13000.00"),
        ],
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "dispose redirects, got {}",
        resp.status()
    );

    let (status,): (String,) = sqlx::query_as("SELECT status FROM fixed_assets WHERE id = $1")
        .bind(asset)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "disposed");

    // The disposal journal balances and books the gain.
    let rows: Vec<(String, Decimal, String)> = sqlx::query_as(
        "SELECT p.direction, p.amount, a.name FROM postings p
         JOIN transactions t ON t.id = p.transaction_id
         JOIN accounts a ON a.id = p.account_id
         WHERE t.ledger_id = $1 AND t.description = 'Disposal of fixed asset'",
    )
    .bind(ledger_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3, "proceeds + NBV relief + gain, got {rows:?}");
    let dr: Decimal = rows
        .iter()
        .filter(|(d, _, _)| d == "DEBIT")
        .map(|(_, a, _)| *a)
        .sum();
    let cr: Decimal = rows
        .iter()
        .filter(|(d, _, _)| d == "CREDIT")
        .map(|(_, a, _)| *a)
        .sum();
    assert_eq!(dr, cr, "disposal journal balances");
    assert_eq!(dr, dec("13000.00"));
    assert!(
        rows.iter().any(|(_, a, _)| *a == dec("1000.00")),
        "gain leg present: {rows:?}"
    );
}
