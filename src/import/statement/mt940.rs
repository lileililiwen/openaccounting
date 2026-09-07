//! MT940/MT942 (SWIFT) statement parsing.
//!
//! Line-oriented tag format: `:61:` statement lines followed by
//! `:86:` information blocks. Amounts use comma decimals and a C/D
//! funds-code; payee names hide behind `?20…` structured fields.

use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::StatementLine;

/// `:61:YYMMDD[MMDD][C|D]amount[,dec]...` — returns (date, signed amount).
fn parse_61(line: &str, fallback_year: i32) -> Result<(NaiveDate, Decimal), String> {
    let body = line.trim_start_matches(":61:");
    if body.len() < 10 {
        return Err(format!("short :61: line '{line}'"));
    }
    let yy: i32 = body[0..2]
        .parse()
        .map_err(|_| format!("bad :61: date '{line}'"))?;
    let year = 2000 + yy;
    let month: u32 = body[2..4]
        .parse()
        .map_err(|_| format!("bad :61: month '{line}'"))?;
    let day: u32 = body[4..6]
        .parse()
        .map_err(|_| format!("bad :61: day '{line}'"))?;
    let date = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("impossible :61: date '{line}'"))?;

    // Funds code C/D follows the value date and an optional to-date
    // (four more digits).
    let rest = &body[6..];
    let rest = if rest.len() > 4 && rest.as_bytes()[..4].iter().all(u8::is_ascii_digit) {
        &rest[4..] // skip the to-date
    } else {
        rest
    };
    let funds = rest.chars().next().ok_or("missing funds code")?;
    // Some banks prefix a '-' even though C/D carries the sign.
    let digits_start = rest[1..].trim_start_matches('-');
    let amount_part: String = digits_start
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .collect();
    if amount_part.is_empty() {
        return Err(format!("no amount in :61: '{line}'"));
    }
    let normalized = amount_part.replace(',', ".");
    let value: Decimal = normalized
        .parse()
        .map_err(|_| format!("bad :61: amount '{amount_part}'"))?;
    let signed = match funds {
        'C' | 'c' => value,
        'D' | 'd' => -value,
        other => return Err(format!("unknown funds code '{other}'")),
    };
    Ok((date, signed))
}

/// Extract a human name from an `:86:` block: prefer `?20`-style
/// structured fields, else the raw text.
fn parse_86(block: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for segment in block.split('?') {
        let seg = segment.trim();
        if seg.len() < 3 {
            continue;
        }
        let code = &seg[..2];
        // 20/25 = remitter name lines, 32/33 = purpose/description.
        if matches!(code, "20" | "25" | "32" | "33") {
            parts.push(seg[2..].trim().to_string());
        }
    }
    if parts.is_empty() {
        let flat: String = block
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if flat.is_empty() {
            None
        } else {
            Some(flat)
        }
    } else {
        Some(parts.join(" "))
    }
}

pub fn parse(text: &str) -> Result<Vec<StatementLine>, String> {
    let mut out = Vec::new();
    let mut current_ref: Option<String> = None;
    let mut pending: Option<(NaiveDate, Decimal)> = None;

    for raw in text.lines() {
        let line = raw.trim_end();
        if line.starts_with(":20:") {
            current_ref = Some(line.trim_start_matches(":20:").trim().to_string());
            continue;
        }
        if line.starts_with(":61:") {
            if let Some((date, amount)) = pending.take() {
                out.push(StatementLine {
                    date,
                    amount,
                    payee: "(unnamed)".into(),
                    memo: None,
                    external_id: current_ref.clone(),
                    currency: None,
                });
            }
            pending = Some(parse_61(line, chrono::Utc::now().year()).map_err(|e| e)?);
            continue;
        }
        if line.starts_with(":86:") {
            if let Some((date, amount)) = pending.take() {
                let info = parse_86(line);
                out.push(StatementLine {
                    date,
                    amount,
                    payee: info.clone().unwrap_or_else(|| "(unnamed)".into()),
                    memo: info,
                    external_id: current_ref.clone(),
                    currency: None,
                });
            }
            continue;
        }
        // Continuation of a multi-line :86:.
        if pending.is_some() && !line.trim().is_empty() && !line.starts_with(':') {
            // Handled by folding into the next :86: via parse_86 on the
            // joined block below — kept simple: continuation lines are
            // appended to the last pushed entry when possible.
            if let Some(last) = out.last_mut() {
                if last.payee != "(unnamed)" {
                    last.payee.push(' ');
                    last.payee.push_str(line.trim());
                }
            }
        }
    }
    if let Some((date, amount)) = pending.take() {
        out.push(StatementLine {
            date,
            amount,
            payee: "(unnamed)".into(),
            memo: None,
            external_id: current_ref,
            currency: None,
        });
    }
    if out.is_empty() {
        return Err("no :61: statement lines found".into());
    }
    Ok(out)
}

use chrono::Datelike;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_swift_fixture_with_structured_86() {
        let mt940 = ":20:REF01\n:25:DEUTDEFF/123456\n:61:2608210821C100,50\n:86:?20ACME GMBH?21SUITE 4?32INVOICE 42\n:61:260822D-42,50\n:86:?20KIOSK?32SNACKS\n-\n";
        let lines = parse(mt940).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].date, NaiveDate::from_ymd_opt(2026, 8, 21).unwrap());
        assert_eq!(lines[0].amount, Decimal::new(10050, 2));
        assert!(lines[0].payee.contains("ACME GMBH"), "{}", lines[0].payee);
        assert_eq!(lines[1].amount, Decimal::new(-4250, 2));
        assert_eq!(lines[1].external_id.as_deref(), Some("REF01"));
    }

    #[test]
    fn debit_without_86_gets_unnamed_payee() {
        let mt940 = ":20:R2\n:61:2601010101D5,00\n-\n";
        let lines = parse(mt940).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount, Decimal::new(-500, 2));
        assert_eq!(lines[0].payee, "(unnamed)");
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse("hello world").is_err());
    }
}
