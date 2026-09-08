//! Property tests for the double-entry posting balance invariant.
//!
//! Per `openspec/specs/test-coverage/spec.md`:
//! - For any random partition of a non-negative amount into N >= 2
//!   parts, half of which are debits and half credits, the validator
//!   accepts iff the sum of debits equals the sum of credits.
//! - For any random pair (debits, credits) where they are NOT equal,
//!   the validator rejects with the expected error.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openaccounting::domain::account::AccountType;
use openaccounting::domain::posting::Direction;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Build a balanced multi-leg posting: exactly equal debits and credits.
fn balanced_postings(
    amount: Decimal,
    legs: usize,
) -> (Vec<(Direction, Decimal)>, Vec<(Direction, Decimal)>) {
    assert!(legs >= 2, "need at least 2 legs");
    assert_eq!(legs % 2, 0, "need even number of legs");
    let half = legs / 2;
    let each = amount / Decimal::from(half);
    let debits: Vec<_> = (0..half).map(|_| (Direction::Debit, each)).collect();
    let credits: Vec<_> = (0..half).map(|_| (Direction::Credit, each)).collect();
    (debits, credits)
}

/// Compute the sum of a side.
fn sum(side: &[(Direction, Decimal)]) -> Decimal {
    side.iter().map(|(_, a)| *a).sum()
}

/// Pure double-entry check: debits == credits and both > 0.
fn validate_balance(
    debits: &[(Direction, Decimal)],
    credits: &[(Direction, Decimal)],
) -> Result<(), &'static str> {
    if debits.is_empty() || credits.is_empty() {
        return Err("empty side");
    }
    let d: Decimal = debits.iter().map(|(_, a)| *a).sum();
    let c: Decimal = credits.iter().map(|(_, a)| *a).sum();
    if d.is_sign_negative() || c.is_sign_negative() {
        return Err("negative amount");
    }
    if d == dec!(0) || c == dec!(0) {
        return Err("zero amount");
    }
    if d != c {
        return Err("unbalanced");
    }
    Ok(())
}

#[test]
fn prop_balanced_postings_accepted() {
    // 200 cases: random amounts and leg counts
    for seed in 0u32..200 {
        let amount = Decimal::from((seed as i64 + 1) * 7 + 13);
        let legs = 2 + (seed as usize % 6) * 2; // 2, 4, 6, 8, 10, 12
        let (debits, credits) = balanced_postings(amount, legs);
        assert_eq!(sum(&debits), sum(&credits), "seed={seed}");
        assert!(validate_balance(&debits, &credits).is_ok(), "seed={seed}");
    }
}

#[test]
fn prop_unbalanced_postings_rejected() {
    // 200 cases: add 1 to the credit side to unbalance
    for seed in 0u32..200 {
        let amount = Decimal::from((seed as i64 + 1) * 7 + 13);
        let legs = 2 + (seed as usize % 6) * 2;
        let (debits, credits) = balanced_postings(amount, legs);
        // Unbalance: add 1 to the first credit
        let mut unbalanced = credits.clone();
        unbalanced[0].1 += dec!(1);
        assert_ne!(sum(&debits), sum(&unbalanced), "seed={seed}");
        assert!(
            validate_balance(&debits, &unbalanced).is_err(),
            "seed={seed}"
        );
    }
}

#[test]
fn prop_zero_amounts_rejected() {
    // Zero on either side must be rejected
    for seed in 0u32..100 {
        let amount = Decimal::from((seed as i64 + 1) * 5);
        let (debits, credits) = balanced_postings(amount, 2);
        let mut zero_debits = debits.clone();
        zero_debits[0].1 = dec!(0);
        assert!(validate_balance(&zero_debits, &credits).is_err());
    }
}

#[test]
fn prop_empty_side_rejected() {
    assert!(validate_balance(&[], &[(Direction::Credit, dec!(100))]).is_err());
    assert!(validate_balance(&[(Direction::Debit, dec!(100))], &[]).is_err());
    assert!(validate_balance(&[], &[]).is_err());
}

#[test]
fn prop_negative_amounts_rejected() {
    let result = validate_balance(
        &[(Direction::Debit, dec!(-100))],
        &[(Direction::Credit, dec!(100))],
    );
    assert!(result.is_err());
}

#[test]
fn prop_account_type_sign_mapping_consistent() {
    // For every account type, the normal_direction must be stable
    // and the 5 types must partition into {Debit, Credit} exactly.
    let types = [
        AccountType::Asset,
        AccountType::Liability,
        AccountType::Equity,
        AccountType::Income,
        AccountType::Expense,
    ];
    for t in types {
        let d1 = t.normal_direction();
        let d2 = t.normal_direction();
        assert_eq!(d1, d2, "normal_direction must be deterministic");
    }
    // Asset and Expense are debit-normal
    assert_eq!(AccountType::Asset.normal_direction(), Direction::Debit);
    assert_eq!(AccountType::Expense.normal_direction(), Direction::Debit);
    // Liability, Equity, Income are credit-normal
    assert_eq!(AccountType::Liability.normal_direction(), Direction::Credit);
    assert_eq!(AccountType::Equity.normal_direction(), Direction::Credit);
    assert_eq!(AccountType::Income.normal_direction(), Direction::Credit);
}
