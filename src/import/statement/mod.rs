//! Bank statement file parsers (`statement-import-formats`).
//!
//! One normalized shape for every format:
//! [`StatementLine`] feeds the existing `bank_statement_lines`
//! pipeline, so reconciliation rules, matching, and dedupe apply
//! unchanged. Format detection is content sniffing — file extensions
//! are ignored (banks misname files constantly).

pub mod camt;
pub mod mt940;
pub mod ofx;
pub mod qif;

use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Ofx,
    /// QuickBooks Online export: OFX with Intuit headers
    /// (`INTU.BID`, `INTUIT` aggregates). Parsed via the OFX path —
    /// no new parser, the SGML shape is identical.
    Qbo,
    Qif,
    Camt,
    Mt940,
}

impl Format {
    pub fn label(&self) -> &'static str {
        match self {
            Format::Ofx => "OFX/QFX",
            Format::Qbo => "QBO",
            Format::Qif => "QIF",
            Format::Camt => "CAMT.052/053",
            Format::Mt940 => "MT940",
        }
    }
}

/// One parsed statement line. `amount` is signed from the account's
/// perspective (positive = credit/inflow).
#[derive(Debug, Clone)]
pub struct StatementLine {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub payee: String,
    pub memo: Option<String>,
    pub external_id: Option<String>,
    /// Present in CAMT/MT940; OFX/QIF rarely carry it.
    pub currency: Option<String>,
}

/// Content-sniff the statement format. Extension is ignored.
/// QBO is checked before OFX: every QBO file carries OFXHEADER too,
/// but the Intuit aggregates (`INTU.BID`, `INTUIT`) mark it as QBO.
pub fn sniff_format(bytes: &[u8]) -> Option<Format> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).to_ascii_uppercase();
    if head.contains("INTU.BID") || head.contains("INTUIT") || head.contains("<INTU") {
        return Some(Format::Qbo);
    }
    if head.contains("OFXHEADER") || head.contains("<OFX>") {
        return Some(Format::Ofx);
    }
    if head.starts_with("!TYPE:")
        || head.starts_with("!ACCOUNT")
        || head.lines().next()?.starts_with('!')
    {
        return Some(Format::Qif);
    }
    // ISO 20022 XML: camt namespace id, BkToCstmrStmt node, or the
    // Ntry entry element (present in every camt.052/053 dialect).
    if head.contains("CAMT:")
        || head.contains("CAMT.")
        || head.contains("BKTOCSTMR")
        || head.contains("<NTRY>")
    {
        return Some(Format::Camt);
    }
    // MT940: tagged lines like `:20:` / `:61:`.
    if head.contains(":20:") || head.contains(":61:") {
        return Some(Format::Mt940);
    }
    None
}

/// Parse any supported format into lines. QIF needs the date-order
/// hint because the format carries no locale metadata.
pub fn parse(
    format: Format,
    text: &str,
    qif_date_order: qif::DateOrder,
) -> Result<Vec<StatementLine>, String> {
    match format {
        Format::Ofx | Format::Qbo => ofx::parse(text).map_err(|e| {
            if format == Format::Qbo {
                format!("QBO: {e}")
            } else {
                e
            }
        }),
        Format::Qif => qif::parse(text, qif_date_order),
        Format::Camt => camt::parse(text),
        Format::Mt940 => mt940::parse(text),
    }
}

/// Normalize a payee for aliasing and fingerprint dedupe: lowercase,
/// collapse every non-alphanumeric run to one space, trim.
pub fn normalize_payee(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_space = true; // leading trim
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffing_beats_lying_extensions() {
        // A CAMT file renamed .csv still parses as CAMT.
        let camt = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08"><BkToCstmrStmt>"#;
        assert_eq!(sniff_format(camt), Some(Format::Camt));
        assert_eq!(sniff_format(b"OFXHEADER:100"), Some(Format::Ofx));
        assert_eq!(sniff_format(b"!Type:Bank\n"), Some(Format::Qif));
        assert_eq!(
            sniff_format(b":20:REF01\n:61:2601010101C1,00"),
            Some(Format::Mt940)
        );
        assert_eq!(sniff_format(b"date,desc,amount\n"), None);
    }

    #[test]
    fn normalize_collapses_punctuation_and_case() {
        assert_eq!(normalize_payee("  ACME--GmbH  #12!"), "acme gmbh 12");
        assert_eq!(normalize_payee("ACME   GmbH"), normalize_payee("acme-gmbh"));
    }

    #[test]
    fn qbo_sniffs_as_qbo_not_ofx() {
        let qbo = b"OFXHEADER:100\nDATA:OFXSGML\n<INTU.BID>12345\n<OFX>\n<STMTTRN>";
        assert_eq!(sniff_format(qbo), Some(Format::Qbo));
        assert_eq!(Format::Qbo.label(), "QBO");
    }

    #[test]
    fn qbo_fixture_parses_via_ofx_path() {
        let qbo = "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\n<INTU.BID>01234\n<OFX>\n\
                   <BANKTRANLIST>\n<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20260821\n\
                   <TRNAMT>250.00\n<FITID>QBO-1\n<NAME>Client Pay Inc\n</STMTTRN>\n\
                   <STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260822\n<TRNAMT>-42.50\n\
                   <FITID>QBO-2\n<NAME>ACME GmbH\n</STMTTRN>\n</BANKTRANLIST>";
        let lines = parse(Format::Qbo, qbo, qif::DateOrder::Us).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].payee, "Client Pay Inc");
        assert_eq!(lines[0].amount, Decimal::new(25000, 2));
        assert_eq!(lines[1].amount, Decimal::new(-4250, 2));
    }

    #[test]
    fn camt_053_three_entries_normalize_with_signs() {
        let camt = r#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02">
<BkToCstmrStmt><Stmt>
<Ntry><BookgDt><Dt>2026-08-21</Dt></BookgDt><Amt Ccy="EUR">-42.50</Amt>
<AddtlNtryInf>ACME GMBH OFFICE SUPPLIES</AddtlNtryInf><AcctSvcrRef>CAMT-1</AcctSvcrRef></Ntry>
<Ntry><BookgDt><Dt>2026-08-22</Dt></BookgDt><Amt Ccy="EUR">1000.00</Amt>
<AddtlNtryInf>CLIENT PAY INVOICE 7</AddtlNtryInf><AcctSvcrRef>CAMT-2</AcctSvcrRef></Ntry>
<Ntry><BookgDt><Dt>2026-08-23</Dt></BookgDt><Amt Ccy="EUR">-5.00</Amt>
<AddtlNtryInf>KIOSK COFFEE</AddtlNtryInf><AcctSvcrRef>CAMT-3</AcctSvcrRef></Ntry>
</Stmt></BkToCstmrStmt></Document>"#;
        let lines = parse(Format::Camt, camt, qif::DateOrder::Us).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].date, NaiveDate::from_ymd_opt(2026, 8, 21).unwrap());
        assert_eq!(lines[0].amount, Decimal::new(-4250, 2));
        assert_eq!(lines[1].amount, Decimal::new(100000, 2));
        assert_eq!(lines[2].date, NaiveDate::from_ymd_opt(2026, 8, 23).unwrap());
        assert_eq!(lines[2].amount, Decimal::new(-500, 2));
    }

    #[test]
    fn unreadable_files_reject_with_cause() {
        let err = parse(Format::Camt, "<Document></Document>", qif::DateOrder::Us)
            .expect_err("empty CAMT must fail");
        assert!(err.contains("Ntry"), "cause must name the problem: {err}");
        let err = parse(
            Format::Qbo,
            "OFXHEADER:100\n<OFX></OFX>",
            qif::DateOrder::Us,
        )
        .expect_err("empty QBO must fail");
        assert!(
            err.starts_with("QBO:"),
            "QBO errors carry the format tag: {err}"
        );
    }
}
