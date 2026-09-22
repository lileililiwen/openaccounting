//! Locale coverage reporter (`u7-localization`, day-1 gate).
//!
//! Compares every locale catalog against the English reference
//! catalog and reports per-language missing keys plus the
//! missing-key percentage.
//!
//! Used by:
//!
//! * `scripts/check_locale_coverage.py` to publish a JSON
//!   artifact and fail CI when any day-1 locale exceeds 5 %
//!   missing keys.
//! * Unit tests under this module to validate the math against
//!   fixture catalogs.
//!
//! The reporter is pure: no I/O, no logging, just `serde_json`
//! in / structs out. Callers load the JSON files and pass the
//! parsed values in.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Per-locale coverage report row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocaleCoverage {
    pub code: String,
    pub total_keys: usize,
    pub present_keys: usize,
    pub missing_keys: Vec<String>,
    pub missing_pct: f64,
}

/// Result of comparing every locale against the reference (English)
/// catalog. `reference_total` is the count of keys in the
/// reference catalog — it is the denominator for every locale's
/// `missing_pct` calculation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoverageReport {
    pub reference_total: usize,
    pub locales: BTreeMap<String, LocaleCoverage>,
}

/// Compute per-locale coverage against the reference catalog.
///
/// * `reference` is the English catalog (the source of truth for
///   what *should* exist).
/// * `others` is `locale_code → catalog_json` for every other
///   locale to evaluate.
///
/// Keys with the `_<meta>` shape (`_meta` block in the JSON)
/// are excluded from the denominator so adding per-locale
/// metadata does not inflate the missing count.
pub fn coverage(
    reference: &HashMap<String, String>,
    others: &HashMap<String, HashMap<String, String>>,
) -> CoverageReport {
    let reference_total = reference.iter().filter(|(k, _)| !is_meta_key(k)).count();
    let mut locales = BTreeMap::new();
    for (code, catalog) in others {
        let present = reference
            .iter()
            .filter(|(k, _)| !is_meta_key(k))
            .filter(|(k, _)| catalog.contains_key(*k))
            .count();
        let missing: Vec<String> = reference
            .keys()
            .filter(|k| !is_meta_key(k))
            .filter(|k| !catalog.contains_key(k.as_str()))
            .cloned()
            .collect();
        let missing_count = reference_total.saturating_sub(present);
        let missing_pct = pct(missing_count, reference_total);
        let total_keys = reference_total;
        locales.insert(
            code.clone(),
            LocaleCoverage {
                code: code.clone(),
                total_keys,
                present_keys: present,
                missing_keys: missing,
                missing_pct,
            },
        );
    }
    CoverageReport {
        reference_total,
        locales,
    }
}

/// A locale whose `missing_pct` exceeds `threshold_pct` is a
/// release-blocking locale. Day-1 locales: en, zh-CN, es, fr, de,
/// ja. English is the reference and is never reported as
/// missing.
pub fn failing_locales(report: &CoverageReport, threshold_pct: f64) -> Vec<&LocaleCoverage> {
    report
        .locales
        .values()
        .filter(|l| l.missing_pct > threshold_pct)
        .collect()
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        (n as f64 / d as f64) * 100.0
    }
}

/// Catalog metadata keys (the `_meta` block) are not user-facing
/// strings; skip them when counting missing keys.
fn is_meta_key(k: &str) -> bool {
    k.starts_with('_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("_meta.code".into(), "en".into());
        m.insert("common.save".into(), "Save".into());
        m.insert("common.cancel".into(), "Cancel".into());
        m.insert("nav.dashboard".into(), "Dashboard".into());
        m
    }

    #[test]
    fn coverage_reports_missing_keys() {
        let reference = en();
        let mut others = HashMap::new();
        let mut de = HashMap::new();
        de.insert("common.save".into(), "Speichern".into());
        de.insert("nav.dashboard".into(), "Übersicht".into());
        // common.cancel is intentionally missing.
        others.insert("de".into(), de);

        let report = coverage(&reference, &others);
        assert_eq!(report.reference_total, 3);
        let de = report.locales.get("de").unwrap();
        assert_eq!(de.total_keys, 3);
        assert_eq!(de.present_keys, 2);
        assert_eq!(de.missing_keys, vec!["common.cancel".to_string()]);
        // 1 / 3 ≈ 33.3 %
        assert!(
            (de.missing_pct - 33.333_333_333_333_336).abs() < 1e-9,
            "missing_pct = {}",
            de.missing_pct
        );
    }

    #[test]
    fn full_coverage_is_zero_pct() {
        let reference = en();
        let mut others = HashMap::new();
        let mut es = HashMap::new();
        es.insert("common.save".into(), "Guardar".into());
        es.insert("common.cancel".into(), "Cancelar".into());
        es.insert("nav.dashboard".into(), "Panel".into());
        others.insert("es".into(), es);

        let report = coverage(&reference, &others);
        let es = report.locales.get("es").unwrap();
        assert_eq!(es.missing_pct, 0.0);
        assert!(es.missing_keys.is_empty());
    }

    #[test]
    fn excludes_meta_keys_from_denominator() {
        let mut reference = en();
        reference.insert("_meta.extras".into(), "anything".into());
        let mut others = HashMap::new();
        let mut fr = HashMap::new();
        fr.insert("common.save".into(), "Enregistrer".into());
        fr.insert("common.cancel".into(), "Annuler".into());
        fr.insert("nav.dashboard".into(), "Tableau".into());
        others.insert("fr".into(), fr);

        let report = coverage(&reference, &others);
        // _meta.code and _meta.extras do not count, so the
        // denominator is still 3, and fr is fully covered.
        assert_eq!(report.reference_total, 3);
        let fr = report.locales.get("fr").unwrap();
        assert_eq!(fr.missing_pct, 0.0);
    }

    #[test]
    fn threshold_flag() {
        let reference = en();
        let mut others = HashMap::new();
        let mut de = HashMap::new();
        de.insert("common.save".into(), "Speichern".into());
        // 2 / 3 missing = 66.6 %.
        others.insert("de".into(), de);
        let report = coverage(&reference, &others);
        let failing = failing_locales(&report, 5.0);
        assert_eq!(failing.len(), 1);
        assert_eq!(failing[0].code, "de");
        let passing = failing_locales(&report, 90.0);
        assert!(passing.is_empty());
    }
}
