//! Generic CSV import. The parser mirrors the 7-column shape
//! the existing importer already understood: date, description,
//! debit, credit, account, payee, reference. It is a pure
//! function over the raw CSV bytes (no temp file, no streaming)
//! so the same `ParsedRow` shape carries through from preview
//! to commit.

use crate::handlers::import::ParsedRow;

/// Parse a 7-column CSV into a flat list of `ParsedRow`s.
/// Empty lines and short rows are tolerated; the `account`,
/// `payee`, and `reference` columns are optional and become
/// `None` if the column is missing or empty.
pub fn parse(content: &str) -> Vec<ParsedRow> {
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let mut rows = Vec::new();
    for result in reader.records() {
        let Ok(record) = result else { continue };
        rows.push(ParsedRow {
            date: record.get(0).unwrap_or("").to_string(),
            description: record.get(1).unwrap_or("").to_string(),
            debit: record.get(2).unwrap_or("").to_string(),
            credit: record.get(3).unwrap_or("").to_string(),
            account: record.get(4).and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            }),
            payee: record.get(5).and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            }),
            reference: record.get(6).and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            }),
            is_duplicate: false,
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_csv() {
        let s = "date,description,debit,credit,account,payee,reference\n\
                 2026-08-01,Latte,5.00,,Other Expense,Starbucks,abc\n";
        let rows = parse(s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-08-01");
        assert_eq!(rows[0].debit, "5.00");
        assert_eq!(rows[0].payee.as_deref(), Some("Starbucks"));
    }

    #[test]
    fn empty_columns_become_none() {
        let s = "date,description,debit,credit,account,payee,reference\n\
                 2026-08-01,NoAccount,1.00,,,,ref1\n";
        let rows = parse(s);
        assert_eq!(rows.len(), 1, "rows: {:?}", rows);
        assert_eq!(rows[0].account, None);
        assert_eq!(rows[0].payee, None);
        assert_eq!(rows[0].reference.as_deref(), Some("ref1"));
    }
}
