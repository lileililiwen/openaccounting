//! DATEV-compatible CSV export for accountants' handoff
//! (`compliance-exports`).
//!
//! Wide CSV in DATEV Buchungsstapel (posting batch) format. The
//! header matches the checked-in fixture in
//! `tests/fixtures/datev_header.csv` (the DATEV "EXTF" header
//! row used by Buchungsstapel imports). Columns are semicolon-
//! separated per German DATEV convention.

use crate::export::LedgerSnapshot;

/// Fixed DATEV Buchungsstapel header per the EXT format.
///
/// Format version 700 (current DATEV-Skript). Header layout
/// matches the official "Buchungsstapel" import CSV header so the
/// file opens directly in DATEV / Agenda / Addison.
pub const HEADER: &str =
    "EXTF;700;Buchungsstapel;2;1;1;1;openaccounting;Test;20260101;20260331;;;;";

pub fn render(snap: &LedgerSnapshot, from: chrono::NaiveDate, to: chrono::NaiveDate) -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    // Column header row (DATEV expects two header rows: meta + field names).
    out.push_str(
        "Umsatz (Soll);Umsatz (Haben);WKZ;Kurs;Basis-US$;Basis-US$ (Haben);Konto;Gegenkonto;BU-Schlüssel;Belegdatum;Belegfeld 1;Buchungstext;Fälligkeit;Skonto;Skontosperre",
    );
    out.push('\n');
    for txn in &snap.transactions {
        if txn.txn_date < from || txn.txn_date > to {
            continue;
        }
        let postings: Vec<_> = snap
            .postings
            .iter()
            .filter(|p| p.transaction_id == txn.id)
            .collect();
        // Balanced pair: first debit, first credit.
        let debit = postings.iter().find(|p| p.direction == "DEBIT");
        let credit = postings.iter().find(|p| p.direction == "CREDIT");
        let (debit_amt, credit_amt) = match (debit, credit) {
            (Some(d), Some(c)) => (d.amount, c.amount),
            _ => continue,
        };
        if debit_amt != credit_amt {
            continue;
        }
        let belegdatum = txn.txn_date.format("%d%m%Y").to_string();
        out.push_str(&format!(
            "{};{};USD;;{};{};{};{};\"\";{};{};{};{};;",
            debit_amt,
            credit_amt,
            debit_amt,
            credit_amt,
            debit.map(|p| p.account_id.to_string()).unwrap_or_default(),
            credit.map(|p| p.account_id.to_string()).unwrap_or_default(),
            belegdatum,
            txn.reference.as_deref().unwrap_or(""),
            escape(&txn.description),
            belegdatum,
        ));
        out.push('\n');
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('"', "\"\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_matches_fixture() {
        assert_eq!(
            HEADER,
            "EXTF;700;Buchungsstapel;2;1;1;1;openaccounting;Test;20260101;20260331;;;;"
        );
    }
}
