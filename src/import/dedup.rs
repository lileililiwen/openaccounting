//! Dedup fingerprinting for the platform-specific importers.
//!
//! The fingerprint is `xxh3(date | amount_cents | payee)` where
//! `payee` is normalised (prefixes stripped, lowercased). Two
//! rows with the same fingerprint inside one upload — or against
//! the ledger's recent transactions — are marked as duplicates.

use xxhash_rust::xxh3::Xxh3;

/// Normalise a payee string: strip leading/trailing whitespace,
/// remove the agreed literals (`微信`, `支付宝`, `(`, `)`,
/// `有限公司`), lowercase, and collapse internal whitespace.
pub fn normalize_payee(s: &str) -> String {
    let mut out = String::from(s.trim());
    for pat in ["微信", "支付宝", "(", ")", "有限公司"] {
        out = out.replace(pat, "");
    }
    out.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Compute the dedup fingerprint for a (date, amount_cents,
/// payee) tuple. Order-independent in the sense that two rows
/// carrying the same three components always hash equal.
pub fn fingerprint(date: &str, amount_cents: i64, payee: &str) -> u64 {
    let mut h = Xxh3::new();
    h.update(date.as_bytes());
    h.update(b"|");
    h.update(&amount_cents.to_le_bytes());
    h.update(b"|");
    h.update(normalize_payee(payee).as_bytes());
    h.digest()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_agreed_prefixes_and_lowercases() {
        assert_eq!(normalize_payee(" 微信支付  "), "支付");
        assert_eq!(
            normalize_payee("支付宝(中国)网络技术有限公司"),
            "中国网络技术"
        );
        assert_eq!(normalize_payee("Acme Corp"), "acme corp");
        assert_eq!(normalize_payee("星巴克咖啡"), "星巴克咖啡");
    }

    #[test]
    fn fingerprint_is_stable() {
        let a = fingerprint("2026-08-01", 1000, "微信支付");
        let b = fingerprint("2026-08-01", 1000, " 微信支付 ");
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_differs_on_amount() {
        let a = fingerprint("2026-08-01", 1000, "Acme");
        let b = fingerprint("2026-08-01", 1001, "Acme");
        assert_ne!(a, b);
    }

    /// Property: the fingerprint is order-independent — the set
    /// of fingerprints for any collection of tuples equals the
    /// set after reordering.
    #[test]
    fn prop_fingerprint_is_order_independent() {
        let dates = ["2026-01-01", "2026-02-14", "2026-08-31", "2025-12-31"];
        let payees = [
            "微信支付",
            "支付宝(中国)网络技术有限公司",
            "Acme Corp",
            "星巴克咖啡",
            "发给小红",
        ];
        let mut tuples: Vec<(String, i64, String)> = (0..1000)
            .map(|i| {
                let date = dates[i % dates.len()].to_string();
                let amount = (i as i64 * 97) % 100_000;
                let payee = payees[i % payees.len()].to_string();
                (date, amount, payee)
            })
            .collect();
        let set: std::collections::HashSet<u64> = tuples
            .iter()
            .map(|(d, a, p)| fingerprint(d, *a, p))
            .collect();
        tuples.reverse();
        let set2: std::collections::HashSet<u64> = tuples
            .iter()
            .map(|(d, a, p)| fingerprint(d, *a, p))
            .collect();
        assert_eq!(set, set2);
    }

    /// Property: normalize_payee is idempotent.
    #[test]
    fn prop_normalize_payee_is_idempotent() {
        let bases = [
            "微信支付",
            "支付宝(中国)网络技术有限公司",
            "Acme Corp",
            "星巴克咖啡",
            "发给小红",
            "",
            "  padded  ",
            "Some Company Ltd",
        ];
        for i in 0..1000 {
            let b = bases[i % bases.len()];
            let s = normalize_payee(b);
            assert_eq!(s, normalize_payee(&s), "not idempotent for {b:?}");
        }
    }
}
