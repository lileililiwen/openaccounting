//! Statement reconciliation sessions (`statement-reconciliation`).
//!
//! Pure domain math, independent of HTTP and templates:
//! `difference = stmt_close_balance - (opening + sum(cleared))`.
//! Finishing requires difference exactly zero (`Decimal` comparison —
//! accountants reconcile to the cent; tolerance windows hide errors).
//! Reopening a closed session requires a reason (min 10 chars) and an
//! audit row (written by the handler, not here).

use rust_decimal::Decimal;

/// Carry the opening balance forward: the new session opens at the
/// prior closed session's closing balance, or zero when there is none.
pub fn carry_opening(prior_close: Option<Decimal>) -> Decimal {
    prior_close.unwrap_or(Decimal::ZERO)
}

/// `difference = stmt_close - (opening + cleared_sum)`.
pub fn compute_difference(opening: Decimal, cleared_sum: Decimal, stmt_close: Decimal) -> Decimal {
    stmt_close - (opening + cleared_sum)
}

/// Zero-gate: `Ok(())` only when the difference is exactly zero.
/// `Err(difference)` carries the blocking amount for the 409 body.
pub fn validate_finish(difference: Decimal) -> Result<(), Decimal> {
    if difference.is_zero() {
        Ok(())
    } else {
        Err(difference)
    }
}

/// Unreconcile reason gate: trimmed reason must be at least 10 chars.
/// `Err(reason)` carries a human message for the 400 body.
pub fn validate_unreconcile_reason(reason: &str) -> Result<(), String> {
    if reason.trim().chars().count() >= 10 {
        Ok(())
    } else {
        Err("unreconcile reason must be at least 10 characters".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn difference_zero_when_cleared_ties_to_close() {
        // opening 12,400.00 + cleared 350.25 - 100.00 = close 12,650.25
        let diff = compute_difference(dec("12400.00"), dec("250.25"), dec("12650.25"));
        assert!(diff.is_zero());
        assert!(validate_finish(diff).is_ok());
    }

    #[test]
    fn difference_nonzero_blocks_finish() {
        // Spec scenario: difference 25.10 blocks finish.
        let diff = compute_difference(dec("12400.00"), dec("100.00"), dec("12525.10"));
        assert_eq!(diff, dec("25.10"));
        assert_eq!(validate_finish(diff), Err(dec("25.10")));
    }

    #[test]
    fn opening_carries_forward_from_prior_close() {
        assert_eq!(carry_opening(Some(dec("12400.00"))), dec("12400.00"));
        assert_eq!(carry_opening(None), Decimal::ZERO);
    }

    #[test]
    fn unreconcile_reason_gate() {
        assert!(validate_unreconcile_reason("").is_err());
        assert!(validate_unreconcile_reason("oops").is_err());
        assert!(validate_unreconcile_reason("nine char").is_err()); // 9 chars
        assert!(validate_unreconcile_reason("ten chars!").is_ok()); // 10 chars
        assert!(validate_unreconcile_reason("  correction: wrong period  ").is_ok());
    }

    #[test]
    fn random_cleared_subsets_stay_decimal_exact() {
        // Property (1.5): no RNG crate — xorshift64 over fixed seeds.
        // Every subset sums exactly; difference recomputed from the
        // subset always equals close - (opening + subset_sum).
        let mut rng: u64 = 0x9E3779B97F4A7C15;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for _ in 0..256 {
            let opening = Decimal::new((next() % 1_000_000) as i64, 2);
            let close = Decimal::new((next() % 1_000_000) as i64, 2);
            let n = 1 + next() % 12;
            let mut lines = Vec::new();
            for _ in 0..n {
                let cents = (next() % 20_000) as i64 - 10_000; // -100.00..100.00
                lines.push(Decimal::new(cents, 2));
            }
            // Random subset via bitmask.
            let mask = next();
            let subset_sum: Decimal = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| (mask >> i) & 1 == 1)
                .map(|(_, a)| *a)
                .sum();
            // Exactness: subset + complement == total, to the cent.
            let complement: Decimal = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| (mask >> i) & 1 != 1)
                .map(|(_, a)| *a)
                .sum();
            let total: Decimal = lines.iter().sum();
            assert_eq!(subset_sum + complement, total);
            // Gate consistency: finish allowed iff subset ties out.
            let diff = compute_difference(opening, subset_sum, close);
            assert_eq!(validate_finish(diff).is_ok(), diff.is_zero());
            assert_eq!(diff, close - opening - subset_sum);
        }
    }
}
