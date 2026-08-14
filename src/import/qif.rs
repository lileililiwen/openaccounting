//! QIF (Quicken Interchange Format) parser. The format is a
//! state machine: each transaction is a run of `Xvalue` lines
//! terminated by `^`. The first non-blank line is a header
//! (`!Type:Bank`) and is skipped.

use crate::handlers::import::ParsedRow;

#[derive(Default, Debug)]
struct Txn {
    date: Option<String>,
    amount: Option<String>,
    payee: Option<String>,
    memo: Option<String>,
    reference: Option<String>,
}

fn normalise_date(s: &str) -> String {
    // QIF dates: `08/14/2026` or `08/14'26`. We use
    // MM/DD/YYYY or MM/DD/YY.
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = s.split(['/', '\'']).collect();
    if parts.len() != 3 {
        return s.to_string();
    }
    let mm = parts[0];
    let dd = parts[1];
    let yy = parts[2];
    let yyyy = if yy.len() == 2 {
        format!("20{yy}")
    } else {
        yy.to_string()
    };
    format!("{yyyy}-{mm}-{dd}")
}

fn push_txn(out: &mut Vec<ParsedRow>, t: &Txn) {
    let Some(date) = t.date.as_deref() else { return };
    let Some(amt) = t.amount.as_deref() else { return };
    let mut amt = amt.trim().to_string();
    if amt.is_empty() {
        return;
    }
    let negative = amt.starts_with('-');
    let positive = amt.starts_with('+');
    if negative || positive {
        amt.remove(0);
    }
    let (debit, credit) = if negative {
        (amt.clone(), String::new())
    } else {
        (String::new(), amt.clone())
    };
    out.push(ParsedRow {
        date: normalise_date(date),
        description: t.memo.clone().unwrap_or_default(),
        debit,
        credit,
        account: None,
        payee: t.payee.clone(),
        reference: t.reference.clone(),
        is_duplicate: false,
    });
}

pub fn parse(input: &str) -> Vec<ParsedRow> {
    let mut out = Vec::new();
    let mut current = Txn::default();
    let mut saw_header = false;
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !saw_header {
            // The first non-blank line is a header like
            // `!Type:Bank`; skip it. (Some files don't have
            // one; in that case, we just start the state
            // machine immediately.)
            if line.starts_with('!') {
                saw_header = true;
                continue;
            }
            saw_header = true;
        }
        if line == "^" {
            push_txn(&mut out, &current);
            current = Txn::default();
            continue;
        }
        // The format is single-letter code followed by value
        // (no separator). Some QIFs use a tab.
        let mut chars = line.chars();
        let code = match chars.next() {
            Some(c) if c.is_ascii_alphabetic() => c,
            _ => continue,
        };
        let value: String = chars.collect();
        let value = value.trim_start_matches(['\t', ' ']).to_string();
        match code {
            'D' => current.date = Some(value),
            'T' | 'U' => current.amount = Some(value), // U is the bank-currency total
            'P' => current.payee = Some(value),
            'M' => current.memo = Some(value),
            'N' => current.reference = Some(value),
            // C (cleared), L (category), A (address) are
            // informational; ignore.
            _ => {}
        }
    }
    // QIF files may not end with `^`; flush any open transaction.
    if current.date.is_some() || current.amount.is_some() {
        push_txn(&mut out, &current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_one_line_qif() {
        let s = "!Type:Bank\nD08/14/2026\nT-12.34\nPSupermarket\nMWeekly shop\nN123\n^\n";
        let rows = parse(s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-08-14");
        assert_eq!(rows[0].debit, "12.34");
        assert_eq!(rows[0].description, "Weekly shop");
        assert_eq!(rows[0].payee.as_deref(), Some("Supermarket"));
        assert_eq!(rows[0].reference.as_deref(), Some("123"));
    }

    #[test]
    fn parses_positive_amount_as_credit() {
        let s = "!Type:Bank\nD08/14/2026\nT+100.00\nPSalary\n^\n";
        let rows = parse(s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].credit, "100.00");
        assert_eq!(rows[0].debit, "");
    }

    #[test]
    fn parses_two_year_date() {
        let s = "!Type:Bank\nD08/14'26\nT-1.00\nPTest\n^\n";
        let rows = parse(s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-08-14");
    }
}
