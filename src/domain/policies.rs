//! Reimbursement policy engine.
//!
//! A policy is one of three kinds:
//! - `category_cap`     — max amount per category per day.
//! - `receipt_required` — lines at or above N require a
//!   receipt document attached to the line.
//! - `per_diem`         — destination-based daily allowance.
//!
//! Policies carry a `severity`: hard = block submit/approve,
//! soft = advisory warning. The handler is the only thing
//! that turns a soft violation into a status code; the
//! evaluator just returns the list.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyKind {
    CategoryCap,
    ReceiptRequired,
    PerDiem,
}

impl PolicyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyKind::CategoryCap => "category_cap",
            PolicyKind::ReceiptRequired => "receipt_required",
            PolicyKind::PerDiem => "per_diem",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "category_cap" => Some(Self::CategoryCap),
            "receipt_required" => Some(Self::ReceiptRequired),
            "per_diem" => Some(Self::PerDiem),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Hard,
    Soft,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Hard => "hard",
            Severity::Soft => "soft",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "hard" => Some(Self::Hard),
            "soft" => Some(Self::Soft),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Policy {
    pub id: Uuid,
    pub name: String,
    pub kind: PolicyKind,
    pub config: serde_json::Value,
    pub severity: Severity,
}

#[derive(Clone, Debug)]
pub struct PolicyLine {
    pub category: String,
    pub txn_date: NaiveDate,
    pub amount: Decimal,
    /// True if the line has a receipt document attached.
    pub has_receipt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// Sum of category X on date Y exceeds the cap.
    CategoryCapOver {
        category: String,
        date: NaiveDate,
        actual: i64,
        cap: i64,
    },
    /// Line amount ≥ N but no receipt attached.
    ReceiptMissing { amount: i64, min_required: i64 },
    /// Per-diem line exceeds the daily allowance.
    PerDiemOver {
        destination: String,
        date: NaiveDate,
        actual: i64,
        daily_rate: i64,
    },
}

impl Violation {
    pub fn is_hard(&self, policy_severity: Severity) -> bool {
        policy_severity == Severity::Hard
    }
}

#[derive(Deserialize)]
pub struct CategoryCapConfig {
    pub category: String,
    pub max_per_day: Decimal,
}

#[derive(Deserialize)]
pub struct ReceiptRequiredConfig {
    pub min_amount: Decimal,
}

#[derive(Deserialize)]
pub struct PerDiemConfig {
    pub destination: String,
    pub daily_rate: Decimal,
}

/// Evaluate a single policy against a set of lines.
pub fn evaluate(policy: &Policy, lines: &[PolicyLine]) -> Vec<Violation> {
    match policy.kind {
        PolicyKind::CategoryCap => eval_category_cap(policy, lines),
        PolicyKind::ReceiptRequired => eval_receipt_required(policy, lines),
        PolicyKind::PerDiem => eval_per_diem(policy, lines),
    }
}

fn eval_category_cap(policy: &Policy, lines: &[PolicyLine]) -> Vec<Violation> {
    let Ok(cfg) = serde_json::from_value::<CategoryCapConfig>(policy.config.clone()) else {
        return vec![];
    };
    // Dedupe by (category, date) — one violation per offending
    // (category, date) pair, not one per line.
    use std::collections::BTreeSet;
    let mut seen: BTreeSet<(String, NaiveDate)> = BTreeSet::new();
    let mut out = Vec::new();
    for line in lines {
        if line.category != cfg.category {
            continue;
        }
        let key = (cfg.category.clone(), line.txn_date);
        if seen.contains(&key) {
            continue;
        }
        let sum: Decimal = lines
            .iter()
            .filter(|l| l.category == cfg.category && l.txn_date == line.txn_date)
            .map(|l| l.amount)
            .sum();
        if sum > cfg.max_per_day {
            seen.insert(key);
            out.push(Violation::CategoryCapOver {
                category: cfg.category.clone(),
                date: line.txn_date,
                actual: decimal_to_cents(sum),
                cap: decimal_to_cents(cfg.max_per_day),
            });
        }
    }
    out
}

fn eval_receipt_required(policy: &Policy, lines: &[PolicyLine]) -> Vec<Violation> {
    let Ok(cfg) = serde_json::from_value::<ReceiptRequiredConfig>(policy.config.clone()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for line in lines {
        if line.amount >= cfg.min_amount && !line.has_receipt {
            out.push(Violation::ReceiptMissing {
                amount: decimal_to_cents(line.amount),
                min_required: decimal_to_cents(cfg.min_amount),
            });
        }
    }
    out
}

fn eval_per_diem(policy: &Policy, lines: &[PolicyLine]) -> Vec<Violation> {
    let Ok(cfg) = serde_json::from_value::<PerDiemConfig>(policy.config.clone()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for line in lines {
        // Only travel lines are checked by per-diem; the
        // handler is responsible for tagging them. Without
        // a `destination` field on the line we can't know
        // where the trip is, so we skip the line. (A future
        // change will add `destination` to the line schema.)
        let _ = cfg.destination.clone();
        let _ = line.txn_date;
    }
    out
}

fn decimal_to_cents(d: Decimal) -> i64 {
    (d * Decimal::from(100))
        .to_string()
        .parse::<f64>()
        .unwrap_or(0.0) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn cap_over_returns_violation() {
        let policy = Policy {
            id: Uuid::new_v4(),
            name: "Meals cap".into(),
            kind: PolicyKind::CategoryCap,
            config: serde_json::json!({"category": "meals", "max_per_day": "200.00"}),
            severity: Severity::Hard,
        };
        let lines = vec![
            PolicyLine {
                category: "meals".into(),
                txn_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
                amount: dec!(150),
                has_receipt: true,
            },
            PolicyLine {
                category: "meals".into(),
                txn_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
                amount: dec!(80),
                has_receipt: true,
            },
        ];
        let v = evaluate(&policy, &lines);
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0], Violation::CategoryCapOver { .. }));
    }

    #[test]
    fn receipt_missing_above_threshold() {
        let policy = Policy {
            id: Uuid::new_v4(),
            name: "Receipt required".into(),
            kind: PolicyKind::ReceiptRequired,
            config: serde_json::json!({"min_amount": "50.00"}),
            severity: Severity::Hard,
        };
        let lines = vec![PolicyLine {
            category: "other".into(),
            txn_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            amount: dec!(80),
            has_receipt: false,
        }];
        let v = evaluate(&policy, &lines);
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0], Violation::ReceiptMissing { .. }));
    }

    #[test]
    fn soft_warning_does_not_block_evaluate() {
        // The evaluate fn doesn't gate on severity; the handler
        // does. So a soft policy still returns violations;
        // the handler surfaces them as a banner instead of 400.
        let policy = Policy {
            id: Uuid::new_v4(),
            name: "Soft cap".into(),
            kind: PolicyKind::CategoryCap,
            config: serde_json::json!({"category": "meals", "max_per_day": "100.00"}),
            severity: Severity::Soft,
        };
        let lines = vec![PolicyLine {
            category: "meals".into(),
            txn_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            amount: dec!(200),
            has_receipt: true,
        }];
        let v = evaluate(&policy, &lines);
        assert_eq!(v.len(), 1);
        // The handler then checks v.is_hard() and returns 400
        // only if severity == Hard.
        assert!(!v[0].is_hard(policy.severity));
    }
}
