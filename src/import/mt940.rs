//! MT940 (SWIFT Customer Statement Message) parser. Each
//! `:61:` line begins a transaction; the immediately-following
//! `:86:` line carries the narrative. The `:60F:` / `:62F:`
//! lines are read for the opening / closing balance footer
//! and not converted into postings.

use crate::handlers::import::ParsedRow;

#[derive(Default, Debug)]
struct Txn {
    value_date: Option<String>,
    direction: Option<char>,
    amount: Option<String>,
    reference: Option<String>,
    narrative: Option<String>,
    payee: Option<String>,
}

impl Txn {
    fn clear(&mut self) {
        *self = Self::default();
    }
}

pub struct Mt940 {
    pub rows: Vec<ParsedRow>,
    pub opening_balance: Option<String>,
    pub closing_balance: Option<String>,
}

pub fn parse(input: &str) -> Mt940 {
    let mut out = Vec::new();
    let mut current = Txn::default();
    let mut opening_balance = None;
    let mut closing_balance = None;
    let mut i = 0usize;
    let lines: Vec<&str> = input.lines().collect();
    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with(":60F:") || line.starts_with(":60M:") {
            opening_balance = Some(line[5..].to_string());
            i += 1;
            continue;
        }
        if line.starts_with(":62F:") || line.starts_with(":62M:") {
            closing_balance = Some(line[5..].to_string());
            i += 1;
            continue;
        }
        if line.starts_with(":61:") {
            // Flush any previous txn first.
            if current.value_date.is_some() {
                push_txn(&mut out, &current);
                current.clear();
            }
            current = parse_61(&line[4..]);
            i += 1;
            // The narrative may be on the same line (rare) or
            // on the next line.
            if i < lines.len() && lines[i].trim().starts_with(":86:") {
                let n = parse_86(&lines[i].trim()[4..]);
                current.narrative = Some(n.text);
                current.payee = n.payee;
                i += 1;
            }
            continue;
        }
        if line.starts_with(":86:") {
            let n = parse_86(&line[4..]);
            current.narrative = Some(n.text);
            current.payee = n.payee;
            i += 1;
            continue;
        }
        // Some banks split a :61: line across two physical
        // lines; treat the next non-tagged line as a
        // continuation.
        if current.value_date.is_some()
            && !line.starts_with(':')
            && !line.is_empty()
        {
            current.narrative = Some(line.to_string());
            i += 1;
            continue;
        }
        i += 1;
    }
    if current.value_date.is_some() {
        push_txn(&mut out, &current);
    }
    Mt940 {
        rows: out,
        opening_balance,
        closing_balance,
    }
}

fn parse_61(body: &str) -> Txn {
    // Format: YYMMDD[MMDD] D|C amount [type] N reference
    // (the brackets are optional; the actual grammar is
    // permissive). We pick the first 6 digits as the value
    // date, the next non-digit as the D/C marker, and the
    // following digits + comma + digits as the amount.
    let s = body.trim();
    let bytes = s.as_bytes();
    let mut idx = 0usize;
    let mut value_date = String::new();
    while idx < bytes.len() && value_date.len() < 6 && bytes[idx].is_ascii_digit() {
        value_date.push(bytes[idx] as char);
        idx += 1;
    }
    // Skip optional MMDD (entry date).
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    let mut direction = None;
    if idx < bytes.len() {
        let c = bytes[idx] as char;
        if c == 'D' || c == 'C' {
            direction = Some(c);
            idx += 1;
        }
    }
    // Amount: digits, optional comma, digits, stopping at the
    // first 'N' (or non-digit / non-comma).
    let amount_start = idx;
    while idx < bytes.len()
        && (bytes[idx].is_ascii_digit() || bytes[idx] == b',')
    {
        idx += 1;
    }
    let amount = s[amount_start..idx].to_string();
    // Reference: skip the `N` if present and read the rest.
    let mut reference = None;
    if idx < bytes.len() && bytes[idx] == b'N' {
        let rest = s[idx + 1..].trim();
        // Bank reference ends at the next "//" or end of line.
        let end = rest.find("//").unwrap_or(rest.len());
        reference = Some(rest[..end].to_string());
    }
    Txn {
        value_date: if value_date.len() == 6 {
            // We have YYMMDD; convert to ISO YYYY-MM-DD.
            Some(format!("20{}-{}-{}", &value_date[0..2], &value_date[2..4], &value_date[4..6]))
        } else {
            None
        },
        // We'll fill date below using the full YYYY-MM-DD form
        // once we have MMDD.
        direction,
        amount: if amount.is_empty() {
            None
        } else {
            Some(amount.replace(',', "."))
        },
        reference,
        narrative: None,
        payee: None,
    }
}

/// Helper to actually format the date. We do this in `parse`
/// because we need both YYMMDD and the optional MMDD entry
/// date.
fn full_date(yn: &str, m6: &str) -> String {
    // yn is "YY" (2 chars) from the value date, m6 is "MMDD".
    if yn.len() != 2 || m6.len() != 4 {
        return String::new();
    }
    format!("20{}-{}-{}", yn, &m6[0..2], &m6[2..4])
}

fn parse_86(body: &str) -> Narrative {
    // The :86: narrative is a sequence of `?NNtext` subfields.
    // We prefer ?32 (merchant name) and ?21 (free text) as the
    // payee; the full concatenation is the memo.
    let mut concatenated = String::new();
    let mut payee: Option<String> = None;
    for sub in split_subfields(body) {
        if let Some(rest) = sub.strip_prefix("?32") {
            payee = Some(rest.trim().to_string());
        } else if payee.is_none() {
            if let Some(rest) = sub.strip_prefix("?21") {
                payee = Some(rest.trim().to_string());
            }
        }
        concatenated.push_str(&sub);
        concatenated.push('/');
    }
    let _ = full_date; // suppress unused
    Narrative {
        text: concatenated.trim_end_matches('/').to_string(),
        payee,
    }
}

struct Narrative {
    text: String,
    payee: Option<String>,
}

fn split_subfields(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(idx) = rest.find('?') {
        if idx > 0 {
            // The text before this `?` is a subfield too;
            // some banks emit a leading subfield.
            let prefix = rest[..idx].trim();
            if !prefix.is_empty() {
                out.push(prefix.to_string());
            }
        }
        // Subfield code is 2 chars.
        if idx + 2 >= rest.len() {
            break;
        }
        let code = &rest[idx..idx + 3]; // "?NN"
        // The next subfield starts at the next `?`.
        let after = &rest[idx + 3..];
        let end = after.find('?').unwrap_or(after.len());
        out.push(format!("{code}{}", &after[..end]));
        rest = &after[end..];
    }
    out
}

fn push_txn(out: &mut Vec<ParsedRow>, t: &Txn) {
    let Some(amount) = t.amount.as_deref() else { return };
    let mut amount = amount.trim().to_string();
    if amount.is_empty() {
        return;
    }
    let negative = t.direction == Some('D');
    if negative {
        amount = format!("-{amount}");
    }
    let (debit, credit) = if amount.starts_with('-') {
        (amount.trim_start_matches('-').to_string(), String::new())
    } else {
        (String::new(), amount.clone())
    };
    out.push(ParsedRow {
        date: t
            .value_date
            .clone()
            .unwrap_or_default(),
        description: t.narrative.clone().unwrap_or_default(),
        debit,
        credit,
        account: None,
        payee: t.payee.clone(),
        reference: t.reference.clone(),
        is_duplicate: false,
        ..Default::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_61_86() {
        let s = ":20:STATEMENT\n\
                 :25:1234567890\n\
                 :60F:C260101EUR100,00\n\
                 :61:2608140814D42,50N024NONREF//Settlement\n\
                 :86: ?20PMT ?21Latte ?32STARBUCKS\n\
                 :62F:C260814EUR57,50\n";
        let out = parse(s);
        assert_eq!(out.rows.len(), 1);
        let r = &out.rows[0];
        assert_eq!(r.date, "2026-08-14");
        assert_eq!(r.debit, "42.50");
        assert_eq!(r.credit, "");
        assert_eq!(r.payee.as_deref(), Some("STARBUCKS"));
        assert_eq!(r.reference.as_deref(), Some("024NONREF"));
    }

    #[test]
    fn parses_credit() {
        let s = ":20:STATEMENT\n:25:1234\n:60F:C260101EUR0,00\n:61:260814C100,00N001\n:86: ?32Acme\n:62F:C260814EUR100,00\n";
        let out = parse(s);
        assert_eq!(out.rows.len(), 1);
        assert_eq!(out.rows[0].credit, "100.00");
        assert_eq!(out.rows[0].debit, "");
    }

    #[test]
    fn prefers_32_payee() {
        let s = ":20:S\n:25:1\n:60F:C260101EUR0,00\n:61:260814D10,00N1\n:86: ?21Some text ?32Acme Inc\n:62F:C260814EUR-10,00\n";
        let out = parse(s);
        assert_eq!(out.rows[0].payee.as_deref(), Some("Acme Inc"));
    }
}
