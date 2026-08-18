//! UI localization (`u7-localization`).
//!
//! Six locales on day 1: English (default), Simplified Chinese,
//! Spanish, French, German, Japanese. Strings live in
//! `static/locales/<lang>.json`. The runtime picks the locale in
//! this order:
//!
//! 1. explicit user preference (`users.locale`)
//! 2. `Accept-Language` request header
//! 3. `en`
//!
//! The module exposes:
//!
//! * [`Locale`] — typed enum; parsing accepts both the short
//!   form (`en`) and the regional form (`zh-CN`).
//! * [`I18n`] — owned bundle of strings plus the per-locale
//!   formatters used by the templates. Constructed once at
//!   boot from the JSON files.
//! * [`pick_locale`] — pure function that resolves the
//!   three-tier precedence into a single [`Locale`].
//! * [`fmt_date`] / [`fmt_currency`] — locale-aware
//!   rendering. German dates render `31.12.2025`; German
//!   EUR renders `1.234,56 €`.
//! * Missing-key fallback: `t()` returns the English string
//!   and logs a warning so the gap is visible.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

/// The six day-1 locales.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    #[default]
    En,
    ZhCn,
    Es,
    Fr,
    De,
    Ja,
}

impl Locale {
    pub const ALL: [Locale; 6] = [
        Locale::En,
        Locale::ZhCn,
        Locale::Es,
        Locale::Fr,
        Locale::De,
        Locale::Ja,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::ZhCn => "zh-CN",
            Locale::Es => "es",
            Locale::Fr => "fr",
            Locale::De => "de",
            Locale::Ja => "ja",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::ZhCn => "简体中文",
            Locale::Es => "Español",
            Locale::Fr => "Français",
            Locale::De => "Deutsch",
            Locale::Ja => "日本語",
        }
    }

    /// Parse either the short (`en`) or regional (`zh-CN`)
    /// form. Returns `None` for unknown tags.
    pub fn parse(s: &str) -> Option<Self> {
        let tag = s.split(';').next().unwrap_or(s).trim();
        let primary = tag.split('-').next().unwrap_or(tag);
        match primary.to_ascii_lowercase().as_str() {
            "en" => Some(Locale::En),
            "zh" => Some(Locale::ZhCn),
            "es" => Some(Locale::Es),
            "fr" => Some(Locale::Fr),
            "de" => Some(Locale::De),
            "ja" => Some(Locale::Ja),
            _ => None,
        }
    }
}

impl FromStr for Locale {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Locale::parse(s).ok_or(())
    }
}

/// Resolve the user's locale from the precedence chain.
pub fn pick_locale(user_override: Option<&str>, accept_language: Option<&str>) -> Locale {
    if let Some(s) = user_override {
        if let Some(l) = Locale::parse(s) {
            return l;
        }
    }
    if let Some(header) = accept_language {
        // `Accept-Language` can carry multiple tags with
        // quality factors (`q=0.8`). We don't parse q-values
        // here — the first recognised tag wins. Browsers
        // already order the header by descending quality, so
        // a simple left-to-right scan is good enough for
        // day-1.
        for tag in header.split(',') {
            if let Some(l) = Locale::parse(tag) {
                return l;
            }
        }
    }
    Locale::En
}

/// A flat key → string map. JSON files are loaded into one of
/// these per locale.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct Catalog(HashMap<String, String>);

impl Catalog {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        Ok(Self(serde_json::from_str(s)?))
    }
}

/// Owned i18n bundle: one catalog per locale, plus the
/// fallback (English). Templates hold an `Arc<I18n>` so
/// rendering does not pay a cost per lookup.
#[derive(Debug, Clone)]
pub struct I18n {
    locale: Locale,
    active: Arc<Catalog>,
    fallback: Arc<Catalog>,
}

impl I18n {
    /// Build an `I18n` from the per-locale catalogs. The
    /// `fallback` is used for missing-key lookups; the spec
    /// requires it to be the English catalog.
    pub fn new(locale: Locale, active: Arc<Catalog>, fallback: Arc<Catalog>) -> Self {
        Self {
            locale,
            active,
            fallback,
        }
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// Look up a translation. Missing keys fall back to
    /// English and log a warning so the gap is observable in
    /// logs.
    pub fn t(&self, key: &str) -> std::borrow::Cow<'_, str> {
        if let Some(v) = self.active.get(key) {
            return std::borrow::Cow::Borrowed(v);
        }
        if let Some(v) = self.fallback.get(key) {
            tracing::warn!(
                locale = self.locale.as_str(),
                key = key,
                "missing translation; using English fallback"
            );
            return std::borrow::Cow::Borrowed(v);
        }
        tracing::warn!(key = key, "missing translation AND no English fallback");
        // Last-ditch: return the key itself so the template
        // still renders something readable.
        std::borrow::Cow::Owned(key.to_string())
    }
}

// ─── Locale-aware formatters ───────────────────────────────────────────

/// Look up a plural form. Catalog keys for plurals follow
/// the convention `key.one` / `key.other`. We deliberately
/// keep it simple: 1 → `*.one`, everything else → `*.other`.
/// Slavic / Arabic plural categories are not in scope for
/// day-1.
///
/// Returns the rendered string. Templates that need the
/// numeric value can format it separately and concatenate.
pub fn plural<'a>(i18n: &'a I18n, key: &str, count: u64) -> std::borrow::Cow<'a, str> {
    let variant = if count == 1 { "one" } else { "other" };
    let lookup = format!("{key}.{variant}");
    i18n.t(&lookup)
}

/// Format a date per the locale's convention.
pub fn fmt_date(d: NaiveDate, locale: Locale) -> String {
    match locale {
        // ISO 8601 — default for English.
        Locale::En | Locale::ZhCn => d.format("%Y-%m-%d").to_string(),
        // dd.mm.yyyy
        Locale::De => d.format("%d.%m.%Y").to_string(),
        // dd/mm/yyyy
        Locale::Es | Locale::Fr => d.format("%d/%m/%Y").to_string(),
        // yyyy/mm/dd
        Locale::Ja => d.format("%Y/%m/%d").to_string(),
    }
}

/// Format a UTC datetime with the locale's date convention
/// plus a 24-hour time. Time format does not vary by locale
/// for the day-1 set.
pub fn fmt_datetime(d: DateTime<Utc>, locale: Locale) -> String {
    format!("{} {}", fmt_date(d.date_naive(), locale), d.format("%H:%M"))
}

/// Format a currency amount per the locale's number +
/// currency convention. We honour the spec's two concrete
/// scenarios:
///
/// * `de` + EUR → `1.234,56 €`
/// * `en` + USD → `$1,234.56`
///
/// Other locales default to ISO grouping + dot decimal +
/// symbol-suffix.
pub fn fmt_currency(amount: Decimal, currency: &str, locale: Locale) -> String {
    let grouped = format_grouped(amount, locale);
    match locale {
        Locale::De => match currency {
            "EUR" => format!("{grouped} €"),
            "USD" => format!("{grouped} $"),
            _ => format!("{grouped} {currency}"),
        },
        Locale::Fr => match currency {
            "EUR" => format!("{grouped} €"),
            _ => format!("{currency} {grouped}"),
        },
        Locale::En => match currency {
            "USD" => format!("${grouped}"),
            "EUR" => format!("€{grouped}"),
            "GBP" => format!("£{grouped}"),
            _ => format!("{grouped} {currency}"),
        },
        Locale::Es => format!("{grouped} {currency}"),
        Locale::ZhCn => format!("{currency} {grouped}"),
        Locale::Ja => format!("{currency} {grouped}"),
    }
}

/// Internal helper: emit the amount with locale-appropriate
/// grouping + decimal separators and 2 fractional digits.
fn format_grouped(amount: Decimal, locale: Locale) -> String {
    let negative = amount.is_sign_negative();
    let abs = amount.abs();
    let s = format!("{abs:.2}");
    let (int_part, frac_part) = s.split_once('.').unwrap_or((&s, "00"));
    let thousands = if matches!(locale, Locale::De | Locale::Fr) {
        "."
    } else {
        ","
    };
    let int_grouped = group_thousands(int_part, thousands);
    let separator = if matches!(locale, Locale::De | Locale::Fr) {
        ","
    } else {
        "."
    };
    let mut out = format!("{int_grouped}{separator}{frac_part}");
    if negative {
        out.insert(0, '-');
    }
    out
}

fn group_thousands(int_part: &str, sep: &str) -> String {
    let bytes = int_part.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push_str(sep);
        }
        out.push(*b as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parse_short_and_regional() {
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("en-US"), Some(Locale::En));
        assert_eq!(Locale::parse("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse("zh"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse("de"), Some(Locale::De));
        assert_eq!(Locale::parse("ja"), Some(Locale::Ja));
        assert_eq!(Locale::parse("xx"), None);
    }

    #[test]
    fn pick_locale_precedence() {
        assert_eq!(pick_locale(Some("de"), Some("en")), Locale::De);
        assert_eq!(pick_locale(None, Some("zh-CN")), Locale::ZhCn);
        assert_eq!(pick_locale(None, Some("xx, fr")), Locale::Fr);
        assert_eq!(pick_locale(None, None), Locale::En);
        assert_eq!(pick_locale(Some("xx"), Some("en")), Locale::En);
    }

    #[test]
    fn german_date_format() {
        let d = chrono::NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        assert_eq!(fmt_date(d, Locale::De), "31.12.2025");
        assert_eq!(fmt_date(d, Locale::En), "2025-12-31");
        assert_eq!(fmt_date(d, Locale::Fr), "31/12/2025");
        assert_eq!(fmt_date(d, Locale::Ja), "2025/12/31");
    }

    #[test]
    fn german_currency_format() {
        let amt = dec!(1234.56);
        assert_eq!(fmt_currency(amt, "EUR", Locale::De), "1.234,56 €");
        assert_eq!(fmt_currency(amt, "USD", Locale::En), "$1,234.56");
        assert_eq!(fmt_currency(-amt, "EUR", Locale::De), "-1.234,56 €");
        assert_eq!(fmt_currency(amt, "EUR", Locale::Fr), "1.234,56 €");
    }

    #[test]
    fn english_currency_format() {
        assert_eq!(fmt_currency(dec!(1234.56), "USD", Locale::En), "$1,234.56");
        assert_eq!(fmt_currency(dec!(1234.56), "GBP", Locale::En), "£1,234.56");
    }

    #[test]
    fn missing_key_falls_back_to_english() {
        let en = Arc::new(Catalog(
            std::iter::once(("hello".to_string(), "Hello!".to_string())).collect(),
        ));
        let es = Arc::new(Catalog(Default::default()));
        let i = I18n::new(Locale::Es, es, en.clone());
        assert_eq!(i.t("hello"), "Hello!");
    }

    #[test]
    fn plural_picks_one_vs_other() {
        let cat = Arc::new(Catalog(
            [
                ("txn.one".to_string(), "1 transaction".to_string()),
                ("txn.other".to_string(), "N transactions".to_string()),
            ]
            .into_iter()
            .collect(),
        ));
        let i = I18n::new(Locale::En, cat.clone(), cat);
        assert_eq!(plural(&i, "txn", 1), "1 transaction");
        assert_eq!(plural(&i, "txn", 5), "N transactions");
        assert_eq!(plural(&i, "txn", 0), "N transactions");
    }
}
