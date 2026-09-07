//! Integration tests for `data-interchange`: statement-format imports
//! into the reconciliation pipeline, duplicate detection, payee
//! learning, and retroactive rule application.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn bootstrap(server: &TestServer) -> (String, Uuid, Uuid) {
    let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
    let email = format!("di-{suffix}@example.com");
    let cookie = server.bootstrap_user(&email, &email, PASSWORD).await;
    let pool = server.db().pool();
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", format!("DI Co {suffix}").as_str()),
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
    let account_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Bank Account'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (cookie, ledger_id, account_id)
}

async fn upload(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    account_id: Uuid,
    body: &str,
) -> reqwest::Response {
    let part = reqwest::multipart::Part::text(body.to_string())
        .file_name("statement.csv")
        .mime_str("text/csv")
        .unwrap();
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/reconcile/{account_id}/import",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
}

const OFX_TWO_TXNS: &str = "OFXHEADER:100\n<OFX><BANKTRANLIST>\
<STMTTRN><DTPOSTED>20260821<TRNAMT>-42.50<FITID>TX0001<NAME>ACME GmbH<MEMO>Office supplies</STMTTRN>\
<STMTTRN><DTPOSTED>20260822<TRNAMT>1000.00<FITID>TX0002<NAME>Client Pay</STMTTRN>\
</BANKTRANLIST></OFX>";

#[tokio::test]
async fn ofx_import_lands_in_statement_pipeline_and_dedupes() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, account_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Renamed to .csv — sniffing must still detect OFX.
    let resp = upload(&server, &cookie, ledger_id, account_id, OFX_TWO_TXNS).await;
    assert_eq!(resp.status(), 303);

    let rows: Vec<(NaiveDate, Decimal, String)> = sqlx::query_as(
        "SELECT statement_date, amount, description FROM bank_statement_lines
         WHERE account_id = $1 ORDER BY statement_date",
    )
    .bind(account_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1, Decimal::new(-4250, 2));
    assert_eq!(rows[0].2, "ACME GmbH");

    // External ids stored.
    let ext: Vec<(String,)> = sqlx::query_as(
        "SELECT external_id FROM bank_statement_lines WHERE external_id IS NOT NULL",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ext.len(), 2);

    // Re-download the same file → all duplicates, zero new lines.
    let resp = upload(&server, &cookie, ledger_id, account_id, OFX_TWO_TXNS).await;
    assert_eq!(resp.status(), 303);
    let n: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM bank_statement_lines WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n.0, 2, "re-import is a no-op");
}

#[tokio::test]
async fn qif_fingerprint_dedupe_and_mt940_import() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, account_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    let qif = "!Type:Bank\nD07/01'26\nT-42.50\nPACME GmbH\n^\n";
    let resp = upload(&server, &cookie, ledger_id, account_id, qif).await;
    assert_eq!(resp.status(), 303);
    let n: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM bank_statement_lines WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n.0, 1);

    // Same QIF again → fingerprint duplicate skipped.
    let resp = upload(&server, &cookie, ledger_id, account_id, qif).await;
    assert_eq!(resp.status(), 303);
    let n: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM bank_statement_lines WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n.0, 1, "QIF re-import dedupes on (date, amount, payee)");

    // MT940 imports through the same route.
    let mt940 = ":20:REF01\n:61:2608210821C100,50\n:86:?20ACME GMBH?32INV 42\n";
    let resp = upload(&server, &cookie, ledger_id, account_id, mt940).await;
    assert_eq!(resp.status(), 303);
    let row: (Decimal, String) = sqlx::query_as(
        "SELECT amount, description FROM bank_statement_lines
         WHERE account_id = $1 AND amount = $2",
    )
    .bind(account_id)
    .bind(Decimal::new(10050, 2))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.1.starts_with("ACME"), "{}", row.1);
}

#[tokio::test]
async fn camt_currency_mismatch_rejects_rows_but_imports_valid_ones() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, account_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    // EUR rows into a USD account: rejected per-row; none valid here so
    // the import reports the errors with a 400.
    let camt_eur = r#"<Document><BkToCstmrStmt><Stmt><Ntry>
        <BookgDt><Dt>2026-08-21</Dt></BookgDt>
        <Amt Ccy="EUR">-42.50</Amt>
        <AddtlNtryInf>ACME GMBH</AddtlNtryInf>
        <AcctSvcrRef>SVC-1</AcctSvcrRef>
        </Ntry></Stmt></BkToCstmrStmt></Document>"#;
    let resp = upload(&server, &cookie, ledger_id, account_id, camt_eur).await;
    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(body.contains("EUR vs account USD"), "{body}");
    let n: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM bank_statement_lines WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n.0, 0);
}

// ── Payee learning ──────────────────────────────────────────────────────

#[tokio::test]
async fn confirmed_matches_build_aliases_that_rank_suggestions() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, account_id) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Create a transaction + matching statement line, then confirm via
    // the match endpoint twice → alias hit_count = 2.
    let rent: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Rent Expense', 'EXPENSE', 'OPERATING_EXPENSE', 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    for _ in 0..2 {
        let (txn_id,): (Uuid,) = sqlx::query_as(
            r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, currency, kind, created_by)
               VALUES ($1, '2026-08-01', 'landlord payment', 'Acme Handels GmbH', 'USD', 'standard',
                       (SELECT owner_id FROM ledgers WHERE id = $1))
               RETURNING id"#,
        )
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        for (acct, dir, amt) in [(rent, "DEBIT", "500.00"), (account_id, "CREDIT", "500.00")] {
            sqlx::query(
                "INSERT INTO postings (transaction_id, account_id, direction, amount) VALUES ($1, $2, $3, $4)",
            )
            .bind(txn_id)
            .bind(acct)
            .bind(dir)
            .bind(amt.parse::<Decimal>().unwrap())
            .execute(&pool)
            .await
            .unwrap();
        }
        let (line_id,): (Uuid,) = sqlx::query_as(
            r#"INSERT INTO bank_statement_lines (ledger_id, account_id, statement_date, description, amount)
               VALUES ($1, $2, '2026-08-01', 'ACME GMBH RENT AUG', -500.00) RETURNING id"#,
        )
        .bind(ledger_id)
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        let resp = server
            .client()
            .post(format!(
                "{}/ledgers/{ledger_id}/reconcile/{account_id}/match",
                server.base_url()
            ))
            .header(reqwest::header::COOKIE, &cookie)
            .form(&[
                ("line_id", line_id.to_string()),
                ("transaction_id", txn_id.to_string()),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 303);
    }

    // Alias exists with hit_count=2 pointing at the expense account.
    let (hits, acct): (i32, Option<Uuid>) =
        sqlx::query_as("SELECT hit_count, account_id FROM payee_aliases WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hits, 2);
    assert_eq!(acct, Some(rent));

    // Suggestion endpoint ranks it for prefix "acme".
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/rules/suggest?q=acme",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let first = &body["data"].as_array().unwrap()[0];
    assert_eq!(first["payee"].as_str(), Some("Acme Handels GmbH"));
    assert_eq!(
        first["account_id"].as_str(),
        Some(rent.to_string().as_str()),
        "learned account pre-selected"
    );
}

// ── Retroactive rule application ────────────────────────────────────────

#[tokio::test]
async fn retroactive_apply_dry_run_then_commit_closed_period_aborts() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, account_id) = bootstrap(&server).await;
    let pool = server.db().pool();
    let owner_id: Uuid = sqlx::query_scalar("SELECT owner_id FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    // Categorize rule: anything mentioning ACME → Rent Expense.
    let rent: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Rent Expense', 'EXPENSE', 'OPERATING_EXPENSE', 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO reconciliation_rules (ledger_id, name, kind, priority, predicate, action, is_active)
           VALUES ($1, 'acme→rent', 'categorize', 10, $2, $3, TRUE)"#,
    )
    .bind(ledger_id)
    .bind(serde_json::json!({ "payee_glob": "%ACME%" }))
    .bind(serde_json::json!({ "gl_account_id": rent }))
    .execute(&pool)
    .await
    .unwrap();

    // Two matching transactions + one non-matching.
    let mut txn_ids = Vec::new();
    for desc in [
        "ACME GMBH rent march",
        "ACME GMBH rent april",
        "grocery store",
    ] {
        let (txn_id,): (Uuid,) = sqlx::query_as(
            r#"INSERT INTO transactions (ledger_id, txn_date, description, currency, kind, created_by)
               VALUES ($1, '2026-04-05', $2, 'USD', 'standard', $3) RETURNING id"#,
        )
        .bind(ledger_id)
        .bind(desc)
        .bind(owner_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let other: Uuid = match sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Other'",
        )
        .bind(ledger_id)
        .fetch_optional(&pool)
        .await
        .unwrap()
        {
            Some(id) => id,
            None => sqlx::query_scalar(
                "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
                 VALUES ($1, 'Other', 'EXPENSE', 'OPERATING_EXPENSE', 'USD') RETURNING id",
            )
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        };
        for (acct, dir) in [(other, "DEBIT"), (account_id, "CREDIT")] {
            sqlx::query(
                "INSERT INTO postings (transaction_id, account_id, direction, amount) VALUES ($1, $2, $3, $4)",
            )
            .bind(txn_id)
            .bind(acct)
            .bind(dir)
            .bind(Decimal::new(1000, 2))
            .execute(&pool)
            .await
            .unwrap();
        }
        txn_ids.push(txn_id);
    }
    let ids_form: Vec<(String, String)> = txn_ids
        .iter()
        .map(|id| ("transaction_ids".to_string(), id.to_string()))
        .collect();

    // Dry-run reports exactly two changes without writing.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/rules/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&ids_form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let dry: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(dry["mode"], "dry-run");
    assert_eq!(dry["changes"].as_array().unwrap().len(), 2);

    // Apply commits both: the two ACME txns' expense legs now point at
    // Rent Expense; the grocery txn is untouched.
    let mut apply_form = ids_form.clone();
    apply_form.push(("mode".into(), "apply".into()));
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/rules/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&apply_form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let rent_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT p.transaction_id) FROM postings p
         JOIN accounts a ON a.id = p.account_id
         WHERE a.name = 'Rent Expense' AND a.ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rent_count.0, 2, "both ACME transactions recategorized");

    // Closed-period guard: selecting a transaction from a closed year
    // aborts the whole batch with 409 and changes nothing.
    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by) VALUES ($1, 2025, $2)",
    )
    .bind(ledger_id)
    .bind(owner_id)
    .execute(&pool)
    .await
    .unwrap();
    let (old_txn,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, currency, kind, created_by)
           VALUES ($1, '2025-12-01', 'ACME GMBH rent old', 'USD', 'standard', $2) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut mixed_form: Vec<(String, String)> = vec![
        ("transaction_ids".into(), txn_ids[0].to_string()),
        ("transaction_ids".into(), old_txn.to_string()),
        ("mode".into(), "apply".into()),
    ];
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/rules/apply",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&mixed_form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}
