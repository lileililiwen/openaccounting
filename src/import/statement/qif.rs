//! QIF (Quicken Interchange Format) parsing.
//!
//! QIF carries no currency and ambiguous dates (`7/1'26` is July 1 in
//! the US and January 7 in Europe), so the caller supplies the date
//! order; a per-file auto-detection overrides when unambiguous.

use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::StatementLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    /// m/d/y (US).
    Us,
    /// d.m.y (EU).
    Eu,
}

/// Decide the order for an ambiguous numeric triple `(a, b, year-ish)`.
fn resolve_order(a: u32, b: u32, hint: DateOrder) -> Option<(u32, u32)> {
    match (a <= 12, b <= 12) {
        // b > 12 can only be a day → a is the month.
        (true, false) => Some((a, b)),
        // a > 12 can only be a day → b is the month.
        (false, true) => Some((b, a)),
        (true, true) => match hint {
            DateOrder::Us => Some((a, b)),
            DateOrder::Eu => Some((b, a)),
        },
        _ => None,
    }
}

/// Parse QIF dates: `m/d'yy`, `d.m.yy`, `m/d-yyyy`, `yyyy-mm-dd`, …
pub fn parse_date(raw: &str, hint: DateOrder) -> Result<NaiveDate, String> {
    // Quicken writes `m/d'yy` — the apostrophe IS the year separator.
    // Normalize it to `/` and drop whitespace/commas.
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',' && *c != '"')
        .map(|c| if c == '\'' { '/' } else { c })
        .collect();

    // ISO first — unambiguous.
    if let Ok(d) = NaiveDate::parse_from_str(&cleaned, "%Y-%m-%d") {
        return Ok(d);
    }

    let parts: Vec<&str> = if cleaned.contains('/') {
        cleaned.split('/').collect()
    } else if cleaned.contains('.') {
        cleaned.split('.').collect()
    } else if cleaned.contains('-') {
        cleaned.split('-').collect()
    } else {
        return Err(format!("unparseable QIF date '{raw}'"));
    };
    if parts.len() != 3 {
        return Err(format!("unparseable QIF date '{raw}'"));
    }
    let nums: Vec<u32> = parts
        .iter()
        .map(|p| {
            p.parse::<u32>()
                .map_err(|_| format!("bad date part in '{raw}'"))
        })
        .collect::<Result<_, _>>()?;

    // Four-digit year first → ISO-like already handled; handle y/m/d.
    if nums[0] > 31 {
        let year = nums[0] as i32;
        let d = NaiveDate::from_ymd_opt(year, nums[1], nums[2])
            .ok_or_else(|| format!("impossible QIF date '{raw}'"))?;
        return Ok(d);
    }

    let (month, day) = resolve_order(nums[0], nums[1], hint)
        .ok_or_else(|| format!("ambiguous QIF date '{raw}'"))?;
    let mut year = nums[2] as i32;
    if year < 100 {
        year += if year < 70 { 2000 } else { 1900 };
    }
    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| format!("impossible QIF date '{raw}'"))
}

pub fn parse(text: &str, hint: DateOrder) -> Result<Vec<StatementLine>, String> {
    let mut out = Vec::new();
    let mut date: Option<NaiveDate> = None;
    let mut amount: Option<Decimal> = None;
    let mut payee = String::new();
    let mut memo: Option<String> = None;
    let mut in_bank_section = false;

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.starts_with('!') {
            // Section header — only bank/cash/credit-card sections carry
            // statement lines.
            let upper = line.to_ascii_uppercase();
            in_bank_section = upper.starts_with("!TYPE:BANK")
                || upper.starts_with("!TYPE:CASH")
                || upper.starts_with("!TYPE:CCARD")
                || upper.starts_with("!TYPE:STMT");
            continue;
        }
        if !in_bank_section {
            continue;
        }
        let mut chars = line.chars();
        let code = chars.next();
        let value: String = chars.collect();
        match code {
            Some('D') => date = Some(parse_date(&value, hint)?),
            Some('T') => {
                let cleaned: String = value
                    .chars()
                    .filter(|c| *c != ',' && c.is_ascii_graphic())
                    .collect();
                amount = Some(
                    cleaned
                        .parse()
                        .map_err(|_| format!("bad QIF amount '{value}'"))?,
                );
            }
            Some('P') => payee = value.trim().to_string(),
            Some('M') => memo = Some(value.trim().to_string()),
            Some('^') => {
                // Record terminator.
                match (date, amount) {
                    (Some(d), Some(a)) => out.push(StatementLine {
                        date: d,
                        amount: a,
                        payee: if payee.is_empty() {
                            "(unnamed)".into()
                        } else {
                            payee.clone()
                        },
                        memo: memo.take(),
                        external_id: None,
                        currency: None,
                    }),
                    _ => return Err("QIF record missing date or amount".into()),
                }
                date = None;
                amount = None;
                payee.clear();
            }
            _ => {}
        }
    }
    if out.is_empty() {
        return Err("no QIF transactions found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_us_dates_and_records() {
        let qif = "!Type:Bank\nD07/01'26\nT-42.50\nPACME GmbH\nMOffice supplies\n^\nD07/02'26\nT1.000,00\nPClient\n^\n";
        let lines = parse(qif, DateOrder::Us).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].date, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
        assert_eq!(lines[0].amount, Decimal::new(-4250, 2));
        assert_eq!(lines[0].payee, "ACME GmbH");
    }

    #[test]
    fn eu_hint_flips_ambiguous_dates() {
        assert_eq!(
            parse_date("07/01'26", DateOrder::Eu).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 7).unwrap()
        );
        assert_eq!(
            parse_date("07/01'26", DateOrder::Us).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
    }

    #[test]
    fn unambiguous_dates_ignore_hint() {
        // 13 can only be a day.
        assert_eq!(
            parse_date("13/01'26", DateOrder::Us).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 13).unwrap()
        );
    }

    #[test]
    fn non_bank_sections_are_skipped() {
        let qif = "!Type:Cat\nEfood\n^\n!Type:Bank\nD01/15'26\nT5.00\nPX\n^\n";
        let lines = parse(qif, DateOrder::Us).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].payee, "X");
    }
}
