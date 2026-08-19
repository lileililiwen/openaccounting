//! WASM in-browser demo for OpenAccounting (`d6-wasm-demo`).
//!
//! A read-only subset of the production binary: we expose a
//! small Rust API that lets the JS bridge load a seeded ledger
//! and render a minimal HTML view of the transactions and
//! accounts. No persistence (writes go to IndexedDB via JS),
//! no auth, no reports — just enough to demo the data model
//! without a server.
//!
//! The full server is unchanged; this is a separate crate.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub code: Option<String>,
    pub name: String,
    pub r#type: String,
    pub subtype: String,
    pub balance: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Posting {
    pub account_id: String,
    pub account_name: String,
    pub amount: f64,
    pub direction: String,
    pub memo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub date: String,
    pub description: String,
    pub payee: Option<String>,
    pub kind: String,
    pub postings: Vec<Posting>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ledger {
    pub name: String,
    pub base_currency: String,
    pub accounts: Vec<Account>,
    pub transactions: Vec<Transaction>,
}

/// Load a ledger from the embedded JSON. Returns a JS object
/// the page can render directly.
#[wasm_bindgen]
pub fn load_demo_ledger() -> Result<JsValue, JsError> {
    let ledger = demo_ledger::seed();
    serde_wasm_bindgen::to_value(&ledger).map_err(|e| JsError::new(&format!("{e}")))
}

/// Sum of all transaction amounts, for the dashboard tile.
#[wasm_bindgen]
pub fn total_movement(ledger_json: JsValue) -> Result<f64, JsError> {
    let ledger: Ledger = serde_wasm_bindgen::from_value(ledger_json)
        .map_err(|e| JsError::new(&format!("{e}")))?;
    let mut total = 0.0;
    for t in &ledger.transactions {
        for p in &t.postings {
            if p.direction == "DEBIT" {
                total += p.amount;
            }
        }
    }
    Ok(total)
}

/// Number of transactions in the ledger, for the dashboard
/// tile.
#[wasm_bindgen]
pub fn transaction_count(ledger_json: JsValue) -> Result<usize, JsError> {
    let ledger: Ledger = serde_wasm_bindgen::from_value(ledger_json)
        .map_err(|e| JsError::new(&format!("{e}")))?;
    Ok(ledger.transactions.len())
}

/// Format a balance as `1,234.56 USD`.
#[wasm_bindgen]
pub fn format_balance(amount: f64, currency: &str) -> String {
    let sign = if amount < 0.0 { "-" } else { "" };
    let abs = amount.abs();
    let whole = abs.trunc() as i64;
    let frac = (abs.fract() * 100.0).round() as i64;
    let mut s = String::new();
    let digits = whole.to_string();
    let chars: Vec<char> = digits.chars().collect();
    for (i, c) in chars.iter().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            s.insert(0, ',');
        }
        s.insert(0, *c);
    }
    format!("{sign}{s}.{:02} {}", frac, currency)
}

/// Banner text used by `index.html`. Required by the demo.
#[wasm_bindgen]
pub fn banner_text() -> String {
    "Demo data; not saved to our servers.".to_string()
}

mod demo_ledger {
    use super::{Account, Ledger, Posting, Transaction};

    /// Build a realistic 110-transaction ledger. Numbers are
    /// stable; accounts follow the seeded chart-of-accounts.
    pub fn seed() -> Ledger {
        let mut accounts = vec![
            Account {
                id: "a-cash".into(),
                code: Some("1000".into()),
                name: "Cash on Hand".into(),
                r#type: "ASSET".into(),
                subtype: "CURRENT_ASSET".into(),
                balance: 0.0,
            },
            Account {
                id: "a-bank".into(),
                code: Some("1010".into()),
                name: "Bank Account".into(),
                r#type: "ASSET".into(),
                subtype: "CURRENT_ASSET".into(),
                balance: 0.0,
            },
            Account {
                id: "a-ar".into(),
                code: Some("1200".into()),
                name: "Accounts Receivable".into(),
                r#type: "ASSET".into(),
                subtype: "CURRENT_ASSET".into(),
                balance: 0.0,
            },
            Account {
                id: "a-ap".into(),
                code: Some("2000".into()),
                name: "Accounts Payable".into(),
                r#type: "LIABILITY".into(),
                subtype: "CURRENT_LIABILITY".into(),
                balance: 0.0,
            },
            Account {
                id: "a-sales".into(),
                code: Some("4000".into()),
                name: "Sales Revenue".into(),
                r#type: "INCOME".into(),
                subtype: "OPERATING_INCOME".into(),
                balance: 0.0,
            },
            Account {
                id: "a-other-inc".into(),
                code: Some("4900".into()),
                name: "Other Income".into(),
                r#type: "INCOME".into(),
                subtype: "OPERATING_INCOME".into(),
                balance: 0.0,
            },
            Account {
                id: "a-software".into(),
                code: Some("5200".into()),
                name: "Software & SaaS".into(),
                r#type: "EXPENSE".into(),
                subtype: "OPERATING_EXPENSE".into(),
                balance: 0.0,
            },
            Account {
                id: "a-travel".into(),
                code: Some("5100".into()),
                name: "Travel & Meals".into(),
                r#type: "EXPENSE".into(),
                subtype: "OPERATING_EXPENSE".into(),
                balance: 0.0,
            },
            Account {
                id: "a-office".into(),
                code: Some("5000".into()),
                name: "Office Supplies".into(),
                r#type: "EXPENSE".into(),
                subtype: "OPERATING_EXPENSE".into(),
                balance: 0.0,
            },
            Account {
                id: "a-marketing".into(),
                code: Some("5300".into()),
                name: "Marketing".into(),
                r#type: "EXPENSE".into(),
                subtype: "OPERATING_EXPENSE".into(),
                balance: 0.0,
            },
            Account {
                id: "a-rent".into(),
                code: Some("5500".into()),
                name: "Rent".into(),
                r#type: "EXPENSE".into(),
                subtype: "OPERATING_EXPENSE".into(),
                balance: 0.0,
            },
        ];

        let mut transactions: Vec<Transaction> = Vec::new();
        let mut next_id = 1u64;
        let start_date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        // Per-month revenue: 3 invoices × 12 months = 36 txns.
        // Per-month expenses: 6 categories × 12 months = 72 txns.
        // Grand total: 108 transactions. Spec requires 100+.
        for month in 0..12 {
            let d = start_date
                + chrono::Duration::days(30 * month)
                + chrono::Duration::days(1);
            // Monthly revenue.
            for (amount, payee, kind) in [
                (2400.0, "Customer A", "Invoice"),
                (1100.0, "Customer B", "Invoice"),
                (320.0, "Customer C", "Service fee"),
            ] {
                let t = Transaction {
                    id: format!("t-{}", next_id),
                    date: d.to_string(),
                    description: format!("{kind} — {payee}"),
                    payee: Some(payee.into()),
                    kind: "standard".into(),
                    postings: vec![
                        Posting {
                            account_id: "a-bank".into(),
                            account_name: "Bank Account".into(),
                            amount,
                            direction: "DEBIT".into(),
                            memo: None,
                        },
                        Posting {
                            account_id: "a-sales".into(),
                            account_name: "Sales Revenue".into(),
                            amount,
                            direction: "CREDIT".into(),
                            memo: None,
                        },
                    ],
                };
                next_id += 1;
                transactions.push(t);
            }
            // Monthly expenses.
            for (amount, acct, payee, desc) in [
                (800.0, "a-rent", "Building Co", "Office rent"),
                (250.0, "a-software", "GitHub", "GitHub subscription"),
                (60.0, "a-software", "Notion", "Notion seats"),
                (140.0, "a-office", "Staples", "Office supplies"),
                (350.0, "a-marketing", "AdWords", "Search ads"),
                (180.0, "a-travel", "Lyft", "Client visit"),
            ] {
                let t = Transaction {
                    id: format!("t-{}", next_id),
                    date: d.to_string(),
                    description: desc.into(),
                    payee: Some(payee.into()),
                    kind: "standard".into(),
                    postings: vec![
                        Posting {
                            account_id: acct.into(),
                            account_name: accounts
                                .iter()
                                .find(|a| a.id == acct)
                                .map(|a| a.name.clone())
                                .unwrap_or_default(),
                            amount,
                            direction: "DEBIT".into(),
                            memo: None,
                        },
                        Posting {
                            account_id: "a-bank".into(),
                            account_name: "Bank Account".into(),
                            amount,
                            direction: "CREDIT".into(),
                            memo: None,
                        },
                    ],
                };
                next_id += 1;
                transactions.push(t);
            }
        }

        // Compute balances.
        for t in &transactions {
            for p in &t.postings {
                if let Some(a) = accounts.iter_mut().find(|a| a.id == p.account_id) {
                    if p.direction == "DEBIT" {
                        a.balance += p.amount;
                    } else {
                        a.balance -= p.amount;
                    }
                }
            }
        }

        Ledger {
            name: "Acme Coffee Roasters — Demo".into(),
            base_currency: "USD".into(),
            accounts,
            transactions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_has_at_least_100_transactions() {
        let l = demo_ledger::seed();
        assert!(l.transactions.len() >= 100, "spec requires 100+");
    }

    #[test]
    fn every_transaction_balances() {
        let l = demo_ledger::seed();
        for t in &l.transactions {
            let debits: f64 = t
                .postings
                .iter()
                .filter(|p| p.direction == "DEBIT")
                .map(|p| p.amount)
                .sum();
            let credits: f64 = t
                .postings
                .iter()
                .filter(|p| p.direction == "CREDIT")
                .map(|p| p.amount)
                .sum();
            assert!(
                (debits - credits).abs() < 0.01,
                "transaction {} unbalanced: {} debits / {} credits",
                t.id,
                debits,
                credits
            );
        }
    }

    #[test]
    fn format_balance_emits_thousands_separator() {
        assert_eq!(format_balance(1234.5, "USD"), "1,234.50 USD");
        assert_eq!(format_balance(-12.0, "EUR"), "-12.00 EUR");
    }
}