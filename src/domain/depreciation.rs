//! Depreciation schedules (`accounting-dimensions`).
//!
//! Straight-line (existing behavior, kept) and double-declining-balance
//! per asset (`fixed_assets.depreciation_method`). Pure math; the handler
//! persists `accumulated_depreciation` and posts disposal gain/loss.

use rust_decimal::Decimal;

/// Straight-line monthly charge: (cost − salvage) / (life × 12).
pub fn straight_line_monthly(cost: Decimal, salvage: Decimal, life_years: i32) -> Decimal {
    if life_years <= 0 {
        return Decimal::ZERO;
    }
    (cost - salvage) / Decimal::from(life_years * 12)
}

/// Double-declining-balance monthly charge: NBV × (2 / life) / 12,
/// rounded half-up to cents. Never charges below salvage — the caller
/// caps with [`cap_at_depreciable`].
pub fn declining_balance_monthly(net_book_value: Decimal, life_years: i32) -> Decimal {
    if life_years <= 0 || net_book_value <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let monthly = net_book_value * Decimal::from(2) / Decimal::from(life_years) / Decimal::from(12);
    monthly.round_dp(2)
}

/// Net book value: cost minus accumulated depreciation.
pub fn net_book_value(cost: Decimal, accumulated: Decimal) -> Decimal {
    cost - accumulated
}

/// Cap a proposed charge so accumulated depreciation never exceeds the
/// depreciable base (cost − salvage). Returns the charge to apply.
pub fn cap_at_depreciable(
    proposed: Decimal,
    accumulated: Decimal,
    cost: Decimal,
    salvage: Decimal,
) -> Decimal {
    let remaining = (cost - salvage - accumulated).max(Decimal::ZERO);
    proposed.min(remaining).max(Decimal::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn straight_line_matches_known_schedule() {
        // 12,000 − 2,000 over 5y → 166.666…/mo.
        let m = straight_line_monthly(dec("12000"), dec("2000"), 5);
        assert_eq!(m.round_dp(2), dec("166.67"));
    }

    #[test]
    fn declining_balance_matches_fixture_table_to_the_cent() {
        // Fixture: cost 12,000, salvage 2,000, life 5y → 40 % p.a.
        // Month 1: 12,000 × 0.40 / 12 = 400.00.
        assert_eq!(declining_balance_monthly(dec("12000.00"), 5), dec("400.00"));
        // Month 2: NBV 11,600 → 386.666… → 386.67.
        assert_eq!(declining_balance_monthly(dec("11600.00"), 5), dec("386.67"));
        // Month 3: NBV 11,213.33 → 373.777… → 373.78.
        assert_eq!(declining_balance_monthly(dec("11213.33"), 5), dec("373.78"));
        // Late life: NBV 2,100 → 70.00, but only 100.00 of depreciable
        // base remains (2,100 − 2,000 salvage) → capped to 100.00.
        assert_eq!(
            cap_at_depreciable(dec("70.00"), dec("9900.00"), dec("12000"), dec("2000")),
            dec("70.00")
        );
        assert_eq!(
            cap_at_depreciable(dec("70.00"), dec("9930.00"), dec("12000"), dec("2000")),
            dec("70.00")
        );
        assert_eq!(
            cap_at_depreciable(dec("200.00"), dec("9900.00"), dec("12000"), dec("2000")),
            dec("100.00")
        );
    }
}
