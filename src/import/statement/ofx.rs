//! OFX v1 (SGML) and v2 (XML) statement parsing.
//!
//! OFX v1 files routinely omit closing tags (`<DTPOSTED>20260101`
//! with no `</DTPOSTED>`), so values run until the next `<`. Both
//! versions share the STMTTRN aggregate shape.

use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::StatementLine;

/// Extract the value of `tag` inside `block`: text between `<tag>`
/// and the next `<` (handles both `<T>v</T>` and SGML `<T>v`).
fn tag_value<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}", tag);
    let start = block.find(&open)? + open.len();
    // Skip an optional `>` or attributes.
    let rest = &block[start..];
    let rest = if let Some(stripped) = rest.strip_prefix('>') {
        stripped
    } else {
        rest
    };
    let end = rest.find('<').unwrap_or(rest.len());
    let v = rest[..end].trim();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// Split the file into STMTTRN…/STMTTRN blocks (case-insensitive).
fn stmttrn_blocks(text: &str) -> Vec<String> {
    let upper = text.to_ascii_uppercase();
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = upper[cursor..].find("<STMTTRN>") {
        let start = cursor + rel + "<STMTTRN>".len();
        let end_rel = upper[start..].find("</STMTTRN>").map(|e| e);
        let end = match end_rel {
            Some(e) => start + e,
            None => (start + 4000).min(text.len()),
        };
        blocks.push(text[start..end].to_string());
        cursor = end;
    }
    blocks
}

/// OFX dates: YYYYMMDD[HHMMSS[.XXX]][zone]
fn parse_ofx_date(raw: &str) -> Option<NaiveDate> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 {
        return None;
    }
    NaiveDate::parse_from_str(&digits[..8], "%Y%m%d").ok()
}

pub fn parse(text: &str) -> Result<Vec<StatementLine>, String> {
    let mut out = Vec::new();
    for block in stmttrn_blocks(text) {
        let date_raw =
            tag_value(&block, "DTPOSTED").ok_or_else(|| "STMTTRN missing DTPOSTED".to_string())?;
        let date = parse_ofx_date(date_raw).ok_or_else(|| format!("bad DTPOSTED '{date_raw}'"))?;
        let amount_raw =
            tag_value(&block, "TRNAMT").ok_or_else(|| "STMTTRN missing TRNAMT".to_string())?;
        let amount: Decimal = amount_raw
            .parse()
            .map_err(|_| format!("bad TRNAMT '{amount_raw}'"))?;
        let payee = tag_value(&block, "NAME")
            .or_else(|| tag_value(&block, "PAYEE"))
            .unwrap_or("(unnamed)")
            .to_string();
        let memo = tag_value(&block, "MEMO").map(str::to_string);
        let external_id = tag_value(&block, "FITID").map(str::to_string);

        out.push(StatementLine {
            date,
            amount,
            payee,
            memo,
            external_id,
            currency: None,
        });
    }
    if out.is_empty() {
        return Err("no STMTTRN transactions found in OFX file".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sgml_v1_without_closing_tags() {
        let ofx = "OFXHEADER:100\nDATA:OFXSGML\n\n<OFX>\n<BANKMSGSRSV1><STMTTRNRS>\
                   <STMTRS><BANKTRANLIST>\n<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260821\n\
                   <TRNAMT>-42.50\n<FITID>TX0001\n<NAME>ACME GmbH\n<MEMO>Office supplies\n\
                   </STMTTRN>\n<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20260822\n<TRNAMT>1000\n\
                   <FITID>TX0002\n<NAME>Client Pay\n</STMTTRN>\n</BANKTRANLIST>";
        let lines = parse(ofx).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].date, NaiveDate::from_ymd_opt(2026, 8, 21).unwrap());
        assert_eq!(lines[0].amount, Decimal::new(-4250, 2));
        assert_eq!(lines[0].payee, "ACME GmbH");
        assert_eq!(lines[0].memo.as_deref(), Some("Office supplies"));
        assert_eq!(lines[0].external_id.as_deref(), Some("TX0001"));
        assert_eq!(lines[1].external_id.as_deref(), Some("TX0002"));
    }

    #[test]
    fn parses_xml_v2_with_closing_tags() {
        let ofx = r#"<?xml version="1.0"?><OFX><BANKTRANLIST>
                     <STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20260821</DTPOSTED>
                     <TRNAMT>-10.00</TRNAMT><FITID>X1</FITID><NAME>Kiosk</NAME></STMTTRN>
                     </BANKTRANLIST></OFX>"#;
        let lines = parse(ofx).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].payee, "Kiosk");
    }

    #[test]
    fn empty_file_is_an_error_not_silent_success() {
        assert!(parse("OFXHEADER:100\n<OFX></OFX>").is_err());
    }
}
