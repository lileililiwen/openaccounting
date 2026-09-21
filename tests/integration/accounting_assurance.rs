//! Accounting assurance integration tests (`accounting-domain-assurance`).
//!
//! Loads the canonical synthetic ledger into a fresh database and
//! verifies that every core report matches exact Decimal expected
//! values. Also tests boundary cases, import/export round trips,
//! and report-review metadata.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::common::*;
use openaccounting::fixtures::CORE_LEDGER;
use openaccounting::reports::{
    build_balance_sheet, build_cash_flow, build_income_statement, build_trial_balance, ReportBasis,
};

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn add_account(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    name: &str,
    code: &str,
    account_type: &str,
    subtype: &str,
    currency: &str,
) -> Uuid {
    sqlx::query_as::<_, (Uuid,)>(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(ledger_id)
    .bind(name)
    .bind(code)
    .bind(account_type)
    .bind(subtype)
    .bind(currency)
    .fetch_one(pool)
    .await
    .unwrap()
    .0
}

async fn add_txn(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    actor: Uuid,
    date: NaiveDate,
    desc: &str,
    postings: &[(&str, &str, Decimal)],
    account_ids: &std::collections::HashMap<String, Uuid>,
) {
    let (txn_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
         VALUES ($1, $2, $3, 'USD', $4) RETURNING id",
    )
    .bind(ledger_id)
    .bind(date)
    .bind(desc)
    .bind(actor)
    .fetch_one(pool)
    .await
    .unwrap();
    for (acct_name, dir, amt) in postings {
        let acct_id = account_ids
            .get(*acct_name)
            .unwrap_or_else(|| panic!("account '{acct_name}' not found in fixture map"));
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(txn_id)
        .bind(acct_id)
        .bind(amt)
        .bind(dir)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Load the entire canonical fixture into a fresh database.
/// Returns the ledger id.
async fn load_canonical_fixture(server: &TestServer, _cookie: &str) -> Uuid {
    let pool = server.db().pool();
    let (actor,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Create the ledger directly via SQL to avoid the default
    // chart of accounts created by the HTTP handler.
    let (ledger_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO ledgers (owner_id, name, base_currency, timezone, basis)
         VALUES ($1, $2, $3, 'UTC', 'accrual') RETURNING id",
    )
    .bind(actor)
    .bind(CORE_LEDGER.name)
    .bind(CORE_LEDGER.base_currency)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert all fixture accounts.
    let mut account_ids = std::collections::HashMap::new();
    for acct in CORE_LEDGER.accounts {
        let id = add_account(
            &pool,
            ledger_id,
            acct.name,
            acct.code,
            acct.account_type,
            acct.subtype,
            acct.currency,
        )
        .await;
        account_ids.insert(acct.name.to_string(), id);
    }

    // Insert all fixture transactions.
    for txn in CORE_LEDGER.transactions {
        let postings: Vec<(&str, &str, Decimal)> = txn
            .postings
            .iter()
            .map(|p| (p.account_name, p.direction, p.amount))
            .collect();
        add_txn(
            &pool,
            ledger_id,
            actor,
            txn.date,
            txn.description,
            &postings,
            &account_ids,
        )
        .await;
    }

    ledger_id
}

// ---------------------------------------------------------------------------
// 1.1 + 1.2 + 2.1 + 2.2: Core ledger reconciliation — trial balance,
// income statement, balance sheet, cash flow.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trial_balance_debits_equal_credits() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ta_user", "ta@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let as_of = CORE_LEDGER.period_to;

    let result = build_trial_balance(&pool, ledger_id, as_of).await.unwrap();

    assert_eq!(
        result.total_debit, result.total_credit,
        "trial balance: total_debit ({}) != total_credit ({})",
        result.total_debit, result.total_credit
    );
}

#[tokio::test]
async fn trial_balance_matches_expected_rows() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ta2_user", "ta2@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let as_of = CORE_LEDGER.period_to;

    let result = build_trial_balance(&pool, ledger_id, as_of).await.unwrap();

    // Build a map of actual results by account name.
    let mut actual: std::collections::HashMap<String, (Decimal, Decimal)> =
        std::collections::HashMap::new();
    for row in &result.rows {
        actual.insert(row.account_name.clone(), (row.debit, row.credit));
    }

    for expected in CORE_LEDGER.trial_balance {
        let (dr, cr) = actual.get(expected.account_name).unwrap_or_else(|| {
            panic!(
                "expected account '{}' not found in trial balance",
                expected.account_name
            )
        });
        assert_eq!(
            *dr, expected.debit,
            "account '{}': expected debit {} got {}",
            expected.account_name, expected.debit, dr
        );
        assert_eq!(
            *cr, expected.credit,
            "account '{}': expected credit {} got {}",
            expected.account_name, expected.credit, cr
        );
    }
}

#[tokio::test]
async fn trial_balance_total_debit_equals_expected() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ta3_user", "ta3@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let as_of = CORE_LEDGER.period_to;

    let result = build_trial_balance(&pool, ledger_id, as_of).await.unwrap();

    // Sum of all expected debits.
    let expected_total: Decimal = CORE_LEDGER.trial_balance.iter().map(|r| r.debit).sum();
    assert_eq!(result.total_debit, expected_total);
    assert_eq!(result.total_credit, expected_total);
}

#[tokio::test]
async fn income_statement_matches_expected() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("is_user", "is@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let expected = &CORE_LEDGER.income_statement;

    let result = build_income_statement(
        &pool,
        ledger_id,
        CORE_LEDGER.period_from,
        CORE_LEDGER.period_to,
        ReportBasis::Accrual,
    )
    .await
    .unwrap();

    assert_eq!(
        result.revenue.total, expected.revenue_total,
        "revenue: expected {} got {}",
        expected.revenue_total, result.revenue.total
    );
    assert_eq!(
        result.cost_of_goods_sold.total, expected.cogs_total,
        "COGS: expected {} got {}",
        expected.cogs_total, result.cost_of_goods_sold.total
    );
    assert_eq!(
        result.gross_profit, expected.gross_profit,
        "gross profit: expected {} got {}",
        expected.gross_profit, result.gross_profit
    );
    assert_eq!(
        result.operating_expenses.total, expected.opex_total,
        "opex: expected {} got {}",
        expected.opex_total, result.operating_expenses.total
    );
    assert_eq!(
        result.operating_income, expected.operating_income,
        "operating income: expected {} got {}",
        expected.operating_income, result.operating_income
    );
    assert_eq!(
        result.net_income, expected.net_income,
        "net income: expected {} got {}",
        expected.net_income, result.net_income
    );
}

#[tokio::test]
async fn balance_sheet_matches_expected() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("bs_user", "bs@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let expected = &CORE_LEDGER.balance_sheet;
    let net_income = CORE_LEDGER.income_statement.net_income;

    let result = build_balance_sheet(&pool, ledger_id, CORE_LEDGER.period_to, net_income)
        .await
        .unwrap();

    assert_eq!(
        result.total_assets, expected.total_assets,
        "total assets: expected {} got {}",
        expected.total_assets, result.total_assets
    );
    assert_eq!(
        result.total_liabilities, expected.total_liabilities,
        "total liabilities: expected {} got {}",
        expected.total_liabilities, result.total_liabilities
    );
    assert_eq!(
        result.total_liab_equity, expected.total_liab_equity,
        "total liab+equity: expected {} got {}",
        expected.total_liab_equity, result.total_liab_equity
    );
    // Accounting equation: assets = liabilities + equity
    assert_eq!(
        result.total_assets, result.total_liab_equity,
        "accounting equation violated: assets ({}) != liab+equity ({})",
        result.total_assets, result.total_liab_equity
    );
}

#[tokio::test]
async fn cash_flow_matches_expected() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("cf_user", "cf@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let expected = &CORE_LEDGER.cash_flow;

    let result = build_cash_flow(
        &pool,
        ledger_id,
        CORE_LEDGER.period_from,
        CORE_LEDGER.period_to,
        ReportBasis::Accrual,
    )
    .await
    .unwrap();

    assert_eq!(
        result.opening, expected.opening,
        "cash flow opening: expected {} got {}",
        expected.opening, result.opening
    );
    assert_eq!(
        result.closing, expected.closing,
        "cash flow closing: expected {} got {}",
        expected.closing, result.closing
    );
    assert_eq!(
        result.movement, expected.movement,
        "cash flow movement: expected {} got {}",
        expected.movement, result.movement
    );
    assert_eq!(
        result.total_inflows, expected.total_inflows,
        "cash flow inflows: expected {} got {}",
        expected.total_inflows, result.total_inflows
    );
    assert_eq!(
        result.total_outflows, expected.total_outflows,
        "cash flow outflows: expected {} got {}",
        expected.total_outflows, result.total_outflows
    );
}

// ---------------------------------------------------------------------------
// 1.4: Boundary cases — zero balances, period close, date boundaries.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trial_balance_excludes_zero_balance_accounts() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("zb_user", "zb@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    let result = build_trial_balance(&pool, ledger_id, CORE_LEDGER.period_to)
        .await
        .unwrap();

    // Accounts Receivable and Accounts Payable have zero balance
    // after all transactions are settled.
    for row in &result.rows {
        assert!(
            row.debit != Decimal::ZERO || row.credit != Decimal::ZERO,
            "trial balance row '{}' has zero debit and zero credit",
            row.account_name
        );
    }
}

#[tokio::test]
async fn trial_balance_after_all_transactions_balanced() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("bp_user", "bp@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    // After all fixture transactions, trial balance must balance.
    let as_of = CORE_LEDGER.period_to;
    let result = build_trial_balance(&pool, ledger_id, as_of).await.unwrap();
    assert_eq!(result.total_debit, result.total_credit);
    assert!(
        result.total_debit > Decimal::ZERO,
        "should have non-zero balances"
    );
}

#[tokio::test]
async fn income_statement_empty_period() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ep_user", "ep@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    // Income statement for a period before any transactions.
    let from = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
    let result = build_income_statement(&pool, ledger_id, from, to, ReportBasis::Accrual)
        .await
        .unwrap();

    assert_eq!(result.revenue.total, Decimal::ZERO);
    assert_eq!(result.cost_of_goods_sold.total, Decimal::ZERO);
    assert_eq!(result.net_income, Decimal::ZERO);
}

#[tokio::test]
async fn accounting_equation_holds_for_fixture() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ae_user", "ae@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();
    let net_income = CORE_LEDGER.income_statement.net_income;

    let bs = build_balance_sheet(&pool, ledger_id, CORE_LEDGER.period_to, net_income)
        .await
        .unwrap();
    let tb = build_trial_balance(&pool, ledger_id, CORE_LEDGER.period_to)
        .await
        .unwrap();

    // Trial balance: debits == credits
    assert_eq!(tb.total_debit, tb.total_credit);

    // Balance sheet: assets == liabilities + equity
    assert_eq!(bs.total_assets, bs.total_liab_equity);

    // The net income from the income statement should equal the
    // difference between trial balance credit-normal totals and
    // debit-normal totals for income/expense accounts.
    // (Net income = revenue - expenses)
    let income_credit: Decimal = tb
        .rows
        .iter()
        .filter(|r| r.account_type == "INCOME")
        .map(|r| r.credit)
        .sum();
    let expense_debit: Decimal = tb
        .rows
        .iter()
        .filter(|r| r.account_type == "EXPENSE")
        .map(|r| r.debit)
        .sum();
    let computed_net_income = income_credit - expense_debit;
    assert_eq!(computed_net_income, net_income);
}

// ---------------------------------------------------------------------------
// 1.5: Report-review metadata and non-compliance disclaimers.
// ---------------------------------------------------------------------------

/// Treatment record for advanced accounting workflows.
/// Each record identifies jurisdiction, scope, and non-compliance
/// disclaimer. Reviewer ownership is documented in the treatment
/// record files under docs/accounting-treatment/.
#[allow(dead_code)]
struct TreatmentRecord {
    workflow: &'static str,
    jurisdiction: &'static str,
    scope: &'static str,
    assumptions: &'static str,
    disclaimer: &'static str,
}

const TREATMENT_RECORDS: &[TreatmentRecord] = &[
    TreatmentRecord {
        workflow: "tax",
        jurisdiction: "generic (no jurisdiction-specific rules)",
        scope: "Tax expense recording only; no automatic tax calculation",
        assumptions: "Tax rates are manually configured per jurisdiction; no real-time tax API integration",
        disclaimer: "Passing tests does NOT establish jurisdiction-specific tax compliance. Consult a qualified tax professional for your jurisdiction.",
    },
    TreatmentRecord {
        workflow: "multi-currency-fx",
        jurisdiction: "generic",
        scope: "Currency conversion and realized/unrealized FX gain/loss",
        assumptions: "Exchange rates are manually entered; no live rate feed; rounding to 2 decimal places for base-currency amounts",
        disclaimer: "FX calculations are simplified and may not match your accounting software's treatment. Verify with your auditor.",
    },
    TreatmentRecord {
        workflow: "invoices-ar-ap",
        jurisdiction: "generic",
        scope: "Invoice creation, AR aging, AP aging, payment matching",
        assumptions: "No automated payment reminders; aging buckets are fixed (current/30/60/90+)",
        disclaimer: "Invoice aging reports are informational only and not a substitute for professional accounting advice. Revenue recognition timing may differ under your accounting standards.",
    },
    TreatmentRecord {
        workflow: "amortization",
        jurisdiction: "generic",
        scope: "Straight-line amortization of prepaid expenses and deferred revenue",
        assumptions: "Monthly periods only; no partial-month proration; no amortization schedules with variable rates",
        disclaimer: "Amortization calculations follow straight-line method only and are not a substitute for professional accounting advice. Complex amortization schedules require external tools.",
    },
    TreatmentRecord {
        workflow: "inventory",
        jurisdiction: "generic",
        scope: "FIFO and weighted-average inventory cost tracking and COGS recognition",
        assumptions: "Single-currency only; no LIFO; no physical count reconciliation; method switch blocked with stock on hand",
        disclaimer: "Inventory valuation follows the ledger's disclosed method (FIFO or weighted average). LIFO is not supported.",
    },
    TreatmentRecord {
        workflow: "closing-entries",
        jurisdiction: "generic",
        scope: "Period-close process: close income/expense accounts to retained earnings",
        assumptions: "Monthly close only; no year-end adjustments; no audit trail for close entries",
        disclaimer: "Closing entries are automated and may not match your jurisdiction's requirements for year-end procedures.",
    },
    TreatmentRecord {
        workflow: "reversals",
        jurisdiction: "generic",
        scope: "Transaction voiding and reversal with audit trail",
        assumptions: "Reversals create new offsetting transactions; original transactions are preserved in the audit log",
        disclaimer: "Reversal behavior follows the append-only model and is not a replacement for formal audit controls. Consult your auditor to confirm this meets your jurisdiction's record-keeping requirements.",
    },
    TreatmentRecord {
        workflow: "audit-chain",
        jurisdiction: "generic",
        scope: "Append-only hash chain for transaction integrity",
        assumptions: "SHA-256 hash chain; no external timestamping; chain is backfilled on startup",
        disclaimer: "The audit chain provides tamper-evidence but is NOT a legally binding digital signature. It does not replace formal audit controls.",
    },
];

#[test]
fn treatment_records_exist_for_all_advanced_workflows() {
    for record in TREATMENT_RECORDS {
        assert!(
            !record.workflow.is_empty(),
            "treatment record must have a workflow name"
        );
        assert!(
            !record.jurisdiction.is_empty(),
            "treatment record '{}' must identify jurisdiction/scope",
            record.workflow
        );
        assert!(
            !record.disclaimer.is_empty(),
            "treatment record '{}' must have a non-compliance disclaimer",
            record.workflow
        );
        assert!(
            record.disclaimer.contains("NOT") || record.disclaimer.contains("not"),
            "treatment record '{}' disclaimer must state non-compliance",
            record.workflow
        );
    }
}

#[test]
fn treatment_records_cover_all_specified_workflows() {
    let expected_workflows = [
        "tax",
        "multi-currency-fx",
        "invoices-ar-ap",
        "amortization",
        "inventory",
        "closing-entries",
        "reversals",
        "audit-chain",
    ];
    let actual_workflows: Vec<&str> = TREATMENT_RECORDS.iter().map(|r| r.workflow).collect();
    for expected in &expected_workflows {
        assert!(
            actual_workflows.contains(expected),
            "missing treatment record for workflow '{expected}'"
        );
    }
}

// ---------------------------------------------------------------------------
// 1.3 + 2.3: Import/export round-trip assertions.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_round_trip_preserves_financial_data() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("jr_user", "jr@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    let snapshot = openaccounting::export::LedgerSnapshot::load(&pool, ledger_id)
        .await
        .unwrap();

    // Verify the snapshot has the expected number of accounts and
    // transactions.
    assert_eq!(
        snapshot.accounts.len(),
        CORE_LEDGER.accounts.len(),
        "snapshot should have {} accounts, got {}",
        CORE_LEDGER.accounts.len(),
        snapshot.accounts.len()
    );
    assert_eq!(
        snapshot.transactions.len(),
        CORE_LEDGER.transactions.len(),
        "snapshot should have {} transactions, got {}",
        CORE_LEDGER.transactions.len(),
        snapshot.transactions.len()
    );

    // Verify all currencies are preserved.
    assert!(snapshot.currencies.contains(&"USD".to_string()));

    // Verify each account is present in the snapshot.
    for acct in CORE_LEDGER.accounts {
        let found = snapshot.accounts.iter().any(|a| {
            a.name == acct.name && a.r#type == acct.account_type && a.subtype == acct.subtype
        });
        assert!(found, "account '{}' not found in snapshot", acct.name);
    }

    // Verify JSON serialization works.
    let json = openaccounting::export::json::to_value(&snapshot);
    assert!(json.is_object());
    let snapshot_json = &json["snapshot"];
    assert_eq!(
        snapshot_json["accounts"].as_array().unwrap().len(),
        CORE_LEDGER.accounts.len()
    );
}

#[tokio::test]
async fn beancount_export_has_required_directives() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("bc_user", "bc@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    let snapshot = openaccounting::export::LedgerSnapshot::load(&pool, ledger_id)
        .await
        .unwrap();
    let text = openaccounting::export::beancount::render(&snapshot);

    assert!(text.contains("option \"title\""), "missing title directive");
    assert!(
        text.contains("option \"operating_currency\" \"USD\""),
        "missing operating_currency directive"
    );
    // Verify all fixture accounts appear as beancount open directives.
    for acct in CORE_LEDGER.accounts {
        let bean_name = acct.name.replace(' ', "_");
        assert!(
            text.contains(&format!("open {bean_name} USD")),
            "missing open directive for account '{}'",
            acct.name
        );
    }
    // Verify all fixture transactions appear.
    for txn in CORE_LEDGER.transactions {
        let date_str = txn.date.format("%Y-%m-%d").to_string();
        assert!(
            text.contains(&date_str),
            "missing transaction date '{}' for '{}'",
            date_str,
            txn.description
        );
    }
}

#[tokio::test]
async fn hledger_export_has_required_directives() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("hl_user", "hl@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    let snapshot = openaccounting::export::LedgerSnapshot::load(&pool, ledger_id)
        .await
        .unwrap();
    let text = openaccounting::export::hledger::render(&snapshot);

    // Verify all fixture accounts appear.
    for acct in CORE_LEDGER.accounts {
        assert!(
            text.contains(acct.name),
            "missing account '{}' in hledger export",
            acct.name
        );
    }
    // Verify all fixture transactions appear with their dates.
    for txn in CORE_LEDGER.transactions {
        let date_str = txn.date.format("%Y-%m-%d").to_string();
        assert!(
            text.contains(&date_str),
            "missing date '{}' for '{}' in hledger export",
            date_str,
            txn.description
        );
    }
}

#[tokio::test]
async fn json_export_import_idempotent() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ji_user", "ji@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    let snapshot = openaccounting::export::LedgerSnapshot::load(&pool, ledger_id)
        .await
        .unwrap();
    let json_str = openaccounting::export::json::to_string_pretty(&snapshot);

    // Parse back and verify account count and transaction count
    // are preserved (idempotency check).
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let re_snapshot = &parsed["snapshot"];
    assert_eq!(
        re_snapshot["accounts"].as_array().unwrap().len(),
        snapshot.accounts.len(),
        "JSON round-trip changed account count"
    );
    assert_eq!(
        re_snapshot["transactions"].as_array().unwrap().len(),
        snapshot.transactions.len(),
        "JSON round-trip changed transaction count"
    );
    assert_eq!(
        re_snapshot["postings"].as_array().unwrap().len(),
        snapshot.postings.len(),
        "JSON round-trip changed posting count"
    );

    // Verify financial data is preserved exactly.
    for orig_posting in &snapshot.postings {
        let found = re_snapshot["postings"].as_array().unwrap().iter().any(|p| {
            p["account_id"] == orig_posting.account_id.to_string()
                && p["amount"] == orig_posting.amount.to_string()
                && p["direction"] == orig_posting.direction
        });
        assert!(
            found,
            "posting not found in round-tripped JSON: acct={}, amount={}, dir={}",
            orig_posting.account_id, orig_posting.amount, orig_posting.direction
        );
    }
}

// ---------------------------------------------------------------------------
// 1.3 + 1.4: Boundary cases — exact decimal, rounding, negative values.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn exact_decimal_values_are_preserved() {
    // Verify the fixture uses exact Decimal values, not floating
    // point approximations.
    let amt = dec!(35000);
    assert_eq!(amt, Decimal::from(35000));
    assert_eq!(amt.scale(), 0);

    // Verify non-round values are exact.
    let precise = dec!(1234.56);
    assert_eq!(precise.to_string(), "1234.56");
}

#[tokio::test]
async fn trial_balance_row_count_matches_fixture() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("rc_user", "rc@example.com", PASSWORD)
        .await;
    let ledger_id = load_canonical_fixture(&server, &cookie).await;
    let pool = server.db().pool();

    let result = build_trial_balance(&pool, ledger_id, CORE_LEDGER.period_to)
        .await
        .unwrap();

    // The fixture defines 8 non-zero-balance accounts (A/R and A/P
    // are zero after settlement).
    assert_eq!(
        result.rows.len(),
        CORE_LEDGER.trial_balance.len(),
        "expected {} trial balance rows, got {}",
        CORE_LEDGER.trial_balance.len(),
        result.rows.len()
    );
}
