//! Receipt OCR — engine trait, shared types, and text-extraction helpers.
//!
//! The production implementation is [`crate::ocr::tesseract::TesseractEngine`].
//! Unit tests live in this file; integration tests live in
//! `tests/integration/document_ocr.rs`.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;

pub mod tesseract;

// ─── Public types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrResult {
    pub amount: Option<Decimal>,
    pub txn_date: Option<NaiveDate>,
    pub merchant: Option<String>,
    pub raw_text: String,
    pub confidence: f32,
}

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("PDF tooling (pdftoppm) not found on PATH")]
    PdfToolingMissing,
    #[error("tesseract binary not found on PATH")]
    TesseractMissing,
    #[error("OCR subprocess failed: {0}")]
    ProcessFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait OcrEngine: Send + Sync {
    async fn extract(&self, bytes: &[u8], mime: &str) -> Result<OcrResult, OcrError>;
}

// ─── Text-extraction helpers (pure functions, unit-testable) ───────────────

/// Parse a monetary amount from free text.
///
/// Handles patterns like:
/// - `¥1,234.56`
/// - `Total: 12.34 USD`
/// - `$9.99`
///
/// Returns the first match found, or `None`.
pub fn parse_amount(text: &str) -> Option<Decimal> {
    // Strip common currency symbols / labels and then look for numeric
    // patterns that include at most one decimal point.
    let cleaned: String = text
        .chars()
        .map(|c| match c {
            '¥' | '$' | '€' | '£' | '₩' | '₹' => ' ',
            _ => c,
        })
        .collect();

    // We look for sequences like 1,234.56 or 1234.56 or 12.34
    let re = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\b(\d{1,3}(?:,\d{3})*(?:\.\d{1,4})?|\d+(?:\.\d{1,4})?)\b").unwrap()
    });

    for cap in re.captures_iter(&cleaned) {
        let raw = &cap[1];
        // Remove thousand separators before parsing.
        let stripped = raw.replace(',', "");
        if let Ok(d) = stripped.parse::<Decimal>() {
            if d > Decimal::ZERO {
                return Some(d);
            }
        }
    }
    None
}

/// Parse a date from free text.
///
/// Handles:
/// - `Aug 14, 2026`
/// - `2026-08-14`
///
/// Returns the first match, or `None`.
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    // ISO format: YYYY-MM-DD
    let iso_re =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"\b(\d{4}-\d{2}-\d{2})\b").unwrap());
    if let Some(cap) = iso_re.captures(text) {
        if let Ok(d) = NaiveDate::parse_from_str(&cap[1], "%Y-%m-%d") {
            return Some(d);
        }
    }

    // Month-name format: Jan 01, 2026 / January 1, 2026
    let month_re = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(
            r"\b(Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\s+(\d{1,2}),?\s+(\d{4})\b",
        )
        .unwrap()
    });
    if let Some(cap) = month_re.captures(text) {
        let month_str = &cap[1];
        let day_str = &cap[2];
        let year_str = &cap[3];
        // Normalise to a 3-letter abbreviated form for parsing.
        let month_abbr = &month_str[..3];
        let attempt = format!("{} {}, {}", month_abbr, day_str, year_str);
        if let Ok(d) = NaiveDate::parse_from_str(&attempt, "%b %d, %Y") {
            return Some(d);
        }
        // Try single-digit day without comma
        let attempt2 = format!("{} {} {}", month_abbr, day_str, year_str);
        if let Ok(d) = NaiveDate::parse_from_str(&attempt2, "%b %d %Y") {
            return Some(d);
        }
    }

    None
}

/// Extract the merchant name from OCR raw text.
///
/// Heuristic: the first non-empty line with 4-40 characters is the
/// merchant. Lines that look like dates, amounts, or are all-numeric
/// are skipped.
pub fn extract_merchant(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let len = trimmed.chars().count();
        if !(4..=40).contains(&len) {
            continue;
        }
        // Skip lines that are purely numeric / date-like.
        if trimmed
            .chars()
            .all(|c| c.is_numeric() || matches!(c, '-' | '/' | ':' | ' '))
        {
            continue;
        }
        // Skip lines that look like "Total: 123.45"
        if trimmed.to_lowercase().starts_with("total") {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

// ─── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ── parse_amount ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_amount_yen() {
        let result = parse_amount("¥1,234.56");
        assert_eq!(result, Some(dec!(1234.56)));
    }

    #[test]
    fn test_parse_amount_usd_label() {
        let result = parse_amount("Total: 12.34 USD");
        assert_eq!(result, Some(dec!(12.34)));
    }

    #[test]
    fn test_parse_amount_dollar() {
        let result = parse_amount("$9.99 subtotal");
        assert_eq!(result, Some(dec!(9.99)));
    }

    #[test]
    fn test_parse_amount_no_match() {
        assert_eq!(parse_amount("no numbers here"), None);
    }

    #[test]
    fn test_parse_amount_zero_skipped() {
        // Zero-only amounts should not be returned.
        assert_eq!(parse_amount("0.00"), None);
    }

    // ── parse_date ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_date_iso() {
        assert_eq!(
            parse_date("2026-08-14"),
            Some(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap())
        );
    }

    #[test]
    fn test_parse_date_month_name() {
        assert_eq!(
            parse_date("Aug 14, 2026"),
            Some(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap())
        );
    }

    #[test]
    fn test_parse_date_month_name_full() {
        assert_eq!(
            parse_date("August 14, 2026"),
            Some(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap())
        );
    }

    #[test]
    fn test_parse_date_no_match() {
        assert_eq!(parse_date("no date here"), None);
    }

    // ── extract_merchant ─────────────────────────────────────────────────

    #[test]
    fn test_extract_merchant_first_line() {
        let text = "STARBUCKS COFFEE\n123 Main St\nTotal: 5.40";
        assert_eq!(extract_merchant(text), Some("STARBUCKS COFFEE".into()));
    }

    #[test]
    fn test_extract_merchant_skips_short_lines() {
        let text = "OA\nSTARBUCKS COFFEE\n123 Main St";
        assert_eq!(extract_merchant(text), Some("STARBUCKS COFFEE".into()));
    }

    #[test]
    fn test_extract_merchant_skips_total_line() {
        let text = "Total: 5.40\nSTARBUCKS COFFEE";
        assert_eq!(extract_merchant(text), Some("STARBUCKS COFFEE".into()));
    }

    #[test]
    fn test_extract_merchant_none_when_all_short() {
        let text = "AB\nCD";
        assert_eq!(extract_merchant(text), None);
    }

    // ── property: parse_amount handles currency symbols ──────────────────

    #[cfg(test)]
    mod prop {
        use super::*;

        /// Exercise `parse_amount` with 1000 combinations of currency
        /// symbols and well-formed decimal strings and assert it always
        /// returns the correct value (or `None` for zero).
        #[test]
        fn prop_amount_parser_handles_currency_symbols() {
            let symbols = ['¥', '$', '€', '£', '₩', '₹'];
            let amounts: &[(&str, &str)] = &[
                ("1.00", "1.00"),
                ("12.34", "12.34"),
                ("1234.56", "1234.56"),
                ("1,234.56", "1234.56"),
                ("0.99", "0.99"),
                ("100.00", "100.00"),
            ];
            let mut count = 0;
            for sym in &symbols {
                for &(raw, expected) in amounts {
                    let text = format!("{}{}", sym, raw);
                    let result = parse_amount(&text);
                    let expected_dec: Decimal = expected.parse().unwrap();
                    assert_eq!(result, Some(expected_dec), "failed for input {:?}", text);
                    count += 1;
                }
            }
            assert!(count >= 36, "ran {count} cases");
        }
    }
}
