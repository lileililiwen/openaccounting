//! Beancount text parser for the PTA importer
//! (`d3-plaintext-export`).
//!
//! Handles the subset the exporter produces plus the common
//! directives found in hand-written books:
//!
//! * `option` / `plugin` / `commodity` / `open` / `close` /
//!   `pad` / `balance` / `note` / `event` / `price` / `document`
//!   / `push` / `pop` / `pushtag` / `poptag` — skipped;
//! * `YYYY-MM-DD * "payee" "narration"` transaction headers with
//!   indented postings (`Account  ±amount  CUR`); price/weight
//!   annotations after `@` / `@@` / `#` / `*` are ignored.
//!
//! Unknown non-indented directives are skipped too (Beancount has
//! an open plugin ecosystem); anything that *looks* like a txn
//! header but cannot be parsed is an error.

use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::pta::{PtaPosting, PtaTxn};
use crate::domain::Direction;

/// Parse Beancount text into [`PtaTxn`]s.
pub fn parse(input: &str) -> anyhow::Result<Vec<PtaTxn>> {
    let mut txns: Vec<PtaTxn> = Vec::new();
    let mut current: Option<PtaTxn> = None;
    let mut line_no = 0usize;

    for raw in input.lines() {
        line_no += 1;
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with(';') {
            continue;
        }

        let indented = line.starts_with(' ') || line.starts_with('\t');
        if indented {
            let Some(txn) = current.as_mut() else {
                anyhow::bail!("line {line_no}: posting without a transaction header");
            };
            if let Some(p) = parse_posting(line.trim_start())? {
                txn.postings.push(p);
            }
            continue;
        }

        // Non-indented line. Close the previous transaction.
        let txn = current.take();
        if let Some(t) = txn {
            txns.push(t);
        }

        // Directive handling.
        let body = line.trim_start();
        if let Some(date_part) = body.split_whitespace().next() {
            if let Ok(date) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                let rest = body[date_part.len()..].trim_start();
                if let Some(t) = parse_txn_header(date, rest) {
                    current = Some(t);
                    continue;
                }
                // Unknown or ignorable directive after a date.
                continue;
            }
            // A non-date, non-indented line that is not a comment:
            // skip (e.g. stray text at the top of a file).
            continue;
        }
    }

    if let Some(t) = current.take() {
        txns.push(t);
    }

    Ok(txns)
}

/// Parse `"payee" "narration"` (or `"narration"`) after the date and
/// flag. Returns `None` when the line is a known non-transaction
/// directive or unparseable.
fn parse_txn_header(date: NaiveDate, rest: &str) -> Option<PtaTxn> {
    // `rest` starts with the flag: `*`, `!`, or `txn`.
    let after_flag = rest
        .strip_prefix('*')
        .or_else(|| rest.strip_prefix('!'))
        .or_else(|| rest.strip_prefix("txn"))
        .or_else(|| rest.strip_prefix("txn "))?;
    let after_flag = after_flag.trim_start();

    let parts = split_beancount_strings(after_flag);
    if parts.is_empty() {
        // A txn with an empty narration; accept with a blank title.
        return Some(PtaTxn {
            date,
            description: String::new(),
            payee: None,
            postings: Vec::new(),
        });
    }

    let (payee, description) = match parts.len() {
        1 => (None, parts[0].clone()),
        2 => (Some(parts[0].clone()), parts[1].clone()),
        _ => (Some(parts[0].clone()), parts[1..].join(" ")),
    };
    Some(PtaTxn {
        date,
        description,
        payee,
        postings: Vec::new(),
    })
}

/// Split a Beancount narration into double-quoted strings, falling
/// back to splitting on whitespace when there are no quotes.
fn split_beancount_strings(s: &str) -> Vec<String> {
    if !s.contains('"') {
        return s.split_whitespace().map(str::to_string).collect();
    }
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_string {
                    out.push(std::mem::take(&mut current));
                }
                in_string = !in_string;
            }
            '\\' => {
                if in_string {
                    if let Some(escaped) = chars.next() {
                        current.push(match escaped {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            other => other,
                        });
                    }
                } else {
                    current.push(c);
                }
            }
            other => {
                if in_string {
                    current.push(other);
                } else if !other.is_whitespace() {
                    // Unquoted token outside a string; append.
                    current.push(other);
                }
            }
        }
    }
    if in_string {
        // Unterminated quote; salvage whatever we had.
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

/// Parse one posting line. `None` for balance/price annotations that
/// follow a posting line (e.g. a `balance` directive line is handled
/// upstream; here we tolerate `Account  -0.00 CUR` style empties by
/// skipping zero-amount lines).
fn parse_posting(line: &str) -> anyhow::Result<Option<PtaPosting>> {
    // Strip a trailing `; comment`.
    let line = match line.find(';') {
        Some(idx) => &line[..idx],
        None => line,
    };
    // Drop price/weight annotations.
    let line = match line.find('@').or_else(|| line.find('#')) {
        Some(idx) => line[..idx].trim_end(),
        None => line.trim_end(),
    };
    let mut tokens = line.split_whitespace();
    let account = tokens.next().unwrap_or_default().to_string();
    let amount_tok = tokens.next();
    let Some(amount_tok) = amount_tok else {
        // A bare account with no amount (Beancount infers it). Our
        // exports never emit these; skip rather than guess.
        return Ok(None);
    };
    // Beancount allows `1,000.00` thousands separators.
    let cleaned: String = amount_tok.chars().filter(|c| *c != ',').collect();
    let signed: Decimal = cleaned
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid amount '{amount_tok}'"))?;
    if signed.is_zero() {
        return Ok(None);
    }
    let (amount, direction) = if signed > Decimal::ZERO {
        (signed, Direction::Debit)
    } else {
        (-signed, Direction::Credit)
    };
    Ok(Some(PtaPosting {
        account_name: account,
        amount,
        direction,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exported_transaction() {
        let input = concat!(
            "1970-01-01 commodity USD\n",
            "2026-08-01 open Cash_on_Hand USD\n",
            "2026-08-01 open Sales_Revenue USD\n",
            "\n",
            "2026-08-01 * \"Mogador\" \"Coffee\"\n",
            "  Cash_on_Hand  42.5000 USD\n",
            "  Sales_Revenue  -42.5000 USD\n",
            "\n",
        );
        let txns = parse(input).unwrap();
        assert_eq!(txns.len(), 1);
        let t = &txns[0];
        assert_eq!(t.date.to_string(), "2026-08-01");
        assert_eq!(t.payee.as_deref(), Some("Mogador"));
        assert_eq!(t.description, "Coffee");
        assert_eq!(t.postings.len(), 2);
        assert_eq!(t.postings[0].direction, Direction::Debit);
        assert_eq!(t.postings[0].amount, Decimal::new(425000, 4));
        assert_eq!(t.postings[1].direction, Direction::Credit);
    }

    #[test]
    fn skips_directives_and_handles_prices() {
        let input = concat!(
            "option \"title\" \"Test\"\n",
            "2026-08-01 balance Assets:Cash 100.00 USD\n",
            "2026-08-01 * \"Acme\" \"Refund\"\n",
            "  Assets:Cash  10.00 USD @ 0.9 EUR\n",
            "  Income:Sales -10.00 USD\n",
        );
        let txns = parse(input).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].postings.len(), 2);
    }

    #[test]
    fn unquoted_narration_is_single_description() {
        let txns = parse("2026-08-01 * Groceries\n").unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].description, "Groceries");
        assert_eq!(txns[0].payee, None);
    }

    #[test]
    fn posting_without_header_is_an_error() {
        assert!(parse("  Cash_on_Hand 1.00 USD\n").is_err());
    }
}
