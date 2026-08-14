//! OFX (Open Financial Exchange) parser. Supports both the
//! legacy SGML (QFX) form and OFX 2.x XML.

use crate::handlers::import::ParsedRow;

#[derive(Default, Debug)]
struct Txn {
    dtposted: Option<String>,
    trnamt: Option<String>,
    name: Option<String>,
    memo: Option<String>,
    fitid: Option<String>,
    trntype: Option<String>,
}

fn push_txn(out: &mut Vec<ParsedRow>, t: &Txn) {
    let Some(date) = t.dtposted.as_deref() else { return };
    let Some(amt) = t.trnamt.as_deref() else { return };
    let mut amt = amt.trim().to_string();
    if amt.is_empty() {
        return;
    }
    // OFX amounts use "." as the decimal separator, no
    // thousands separator; sign is leading. Negative = debit.
    let negative = amt.starts_with('-');
    if negative {
        amt.remove(0);
    }
    let description = t
        .memo
        .clone()
        .filter(|m| !m.trim().is_empty())
        .or_else(|| t.name.clone())
        .unwrap_or_default();
    let payee = t.name.clone();
    let reference = t.fitid.clone();

    // Decide direction from the sign and the trntype label.
    // trntype is informational; the sign of the amount is the
    // authority.
    let (debit, credit) = if negative {
        (amt.clone(), String::new())
    } else {
        (String::new(), amt.clone())
    };
    let _ = t.trntype.as_ref(); // trntype kept for future use

    out.push(ParsedRow {
        date: date.to_string(),
        description,
        debit,
        credit,
        account: None,
        payee,
        reference,
        is_duplicate: false,
        ..Default::default()
    });
}

/// Normalise the OFX date (YYYYMMDD or YYYYMMDDHHMMSS or
/// YYYYMMDDHHMMSS.XXX[offset:tz]) to ISO `YYYY-MM-DD`. Accepts
/// the bracketed "[...]" suffix some SGML exports add.
fn normalise_date(s: &str) -> String {
    let s = s.trim().trim_end_matches(']').trim();
    if s.len() < 8 {
        return s.to_string();
    }
    format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
}

pub fn parse_sgml(input: &str) -> Vec<ParsedRow> {
    let mut out = Vec::new();
    let mut current = Txn::default();
    let mut in_stmt = false;
    let normalised = normalise_sgml(input);
    for raw_line in normalised.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // A line may carry several tags if the export is
        // un-pretty-printed. Split on `</TAG>` so each tag
        // gets its own logical line.
        let mut chunks: Vec<String> = Vec::new();
        let mut rest = trimmed.to_string();
        loop {
            if let Some(close_idx) = rest.find("</") {
                // Find the matching `>` of the close tag.
                if let Some(end_idx) = rest[close_idx..].find('>') {
                    chunks.push(rest[..close_idx + end_idx + 1].to_string());
                    rest = rest[close_idx + end_idx + 1..].to_string();
                } else {
                    chunks.push(rest.clone());
                    break;
                }
            } else {
                chunks.push(rest.clone());
                break;
            }
        }
        for line in chunks {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("OFXHEADER:") || upper == "<OFX>" {
                in_stmt = false;
                continue;
            }
            if upper.starts_with("<STMTTRN>") {
                in_stmt = true;
                current = Txn::default();
                continue;
            }
            if upper.starts_with("</STMTTRN>") {
                push_txn(&mut out, &current);
                in_stmt = false;
                continue;
            }
            if !in_stmt {
                continue;
            }
            if let Some((tag, value)) = parse_sgml_value(line) {
                let upper = tag.to_ascii_uppercase();
                match upper.as_str() {
                    "DTPOSTED" => current.dtposted = Some(normalise_date(&value)),
                    "TRNAMT" => current.trnamt = Some(value),
                    "NAME" => current.name = Some(value),
                    "MEMO" => current.memo = Some(value),
                    "FITID" => current.fitid = Some(value),
                    "TRNTYPE" => current.trntype = Some(value),
                    _ => {}
                }
            }
        }
    }
    out
}

/// Insert a newline after every closing `>` (the `>` that
/// follows `</` or a self-closing `/>`). Leaves a single line
/// for `<TAG>value</TAG>` so the per-line value parser sees
/// the whole pair.
fn normalise_sgml(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 64);
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut last_was_open = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '<' {
            // If the previous char was a `>`, we've just closed
            // a tag; insert a newline so the next tag starts on
            // its own line. (Skip if this `<` is itself a
            // closing tag — those always follow the previous
            // opener, never start a new logical line.)
            if last_was_open && i > 0 && bytes[i + 1] != b'/' {
                out.push('\n');
            }
        }
        out.push(c);
        last_was_open = c == '>';
        i += 1;
    }
    out
}

/// SGML value lines can look like:
///   `TRNAMT:-42.50`
///   `MEMO:Latte with milk`
///   `DTPOSTED 20260814`  (some QFX exports use whitespace)
///   `<DTPOSTED>20260814</DTPOSTED>`  (XML-ish in SGML files)
///   `<TRNTYPE>DEBIT</TRNTYPE>` (value between tag markers)
fn parse_sgml_value(line: &str) -> Option<(&str, String)> {
    let trimmed = line.trim();

    // XML-ish form: <TAG>value</TAG> or <TAG/>.
    if let Some(rest) = trimmed.strip_prefix('<') {
        if let Some(end) = rest.find('>') {
            let tag = &rest[..end];
            let tag = tag.trim_end_matches('/');
            if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphabetic()) {
                let after = &rest[end + 1..];
                if let Some(close) = after.find("</") {
                    let value = after[..close].to_string();
                    return Some((tag, value));
                }
                // Self-closing or no value.
                return Some((tag, String::new()));
            }
        }
    }

    if let Some(idx) = line.find(':') {
        let tag = &line[..idx];
        let value = line[idx + 1..].trim().to_string();
        if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some((tag, value));
        }
    }
    if let Some(idx) = line.find(|c: char| c.is_whitespace()) {
        let tag = &line[..idx];
        let value = line[idx..].trim().to_string();
        if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some((tag, value));
        }
    }
    None
}

/// OFX 2.x XML. Implemented with a tiny hand-rolled scanner to
/// avoid pulling in `quick-xml` for a few fields. The parser
/// walks `<STMTTRN>…</STMTTRN>` (or the bank-transaction tag in
/// 2.x) and pulls the same fields the SGML parser pulls.
pub fn parse_xml(input: &str) -> Vec<ParsedRow> {
    let mut out = Vec::new();
    // Strip XML declaration / prolog.
    let body = if let Some(idx) = input.find("<?xml") {
        // Find the closing "?>" and resume from there.
        match input[idx..].find("?>") {
            Some(end) => &input[idx + end + 2..],
            None => input,
        }
    } else {
        input
    };
    // Find each <STMTTRN>…</STMTTRN> region.
    let mut cursor = 0usize;
    while let Some(open) = find_ci(body, cursor, "<STMTTRN>") {
        let close = find_ci(body, open + 9, "</STMTTRN>")
            .unwrap_or_else(|| body.len());
        let region = &body[open + 9..close];
        let mut current = Txn::default();
        for child in extract_elements(region) {
            let (tag, value) = child;
            match tag.to_ascii_uppercase().as_str() {
                "DTPOSTED" => current.dtposted = Some(normalise_date(&value)),
                "TRNAMT" => current.trnamt = Some(value),
                "NAME" => current.name = Some(value),
                "MEMO" => current.memo = Some(value),
                "FITID" => current.fitid = Some(value),
                "TRNTYPE" => current.trntype = Some(value),
                _ => {}
            }
        }
        push_txn(&mut out, &current);
        cursor = close + 10;
    }
    out
}

fn find_ci(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    haystack[from..]
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
        .map(|o| from + o)
}

fn extract_elements(region: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    let bytes = region.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' && i + 1 < bytes.len() && bytes[i + 1] != b'/' && bytes[i + 1] != b'!' {
            // Find end of opening tag.
            if let Some(end) = region[i..].find('>') {
                let tag_raw = &region[i + 1..i + end];
                let tag = tag_raw.split_whitespace().next().unwrap_or("");
                if tag.is_empty() {
                    i += end + 1;
                    continue;
                }
                // Self-closing: <TAG/>
                if tag_raw.ends_with('/') {
                    out.push((tag, String::new()));
                    i += end + 1;
                    continue;
                }
                // Closing tag for this opener: </TAG>
                let close_marker = format!("</{tag}>");
                if let Some(close_idx) = region[i + end + 1..].find(&close_marker) {
                    let value = region[i + end + 1..i + end + 1 + close_idx].trim();
                    out.push((tag, value.to_string()));
                    i += end + 1 + close_idx + close_marker.len();
                    continue;
                }
                i += end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

pub fn parse(input: &str) -> Vec<ParsedRow> {
    if input.contains("<?xml") {
        parse_xml(input)
    } else {
        parse_sgml(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qfx_sgml() {
        let s = "OFXHEADER:100\nDATA:OFXSGML\n<OFX><BANKMSGSRSV1><STMTTRNRS><TRNUID>1</TRNUID><STATUS><CODE>0</CODE><SEVERITY>INFO</SEVERITY></STATUS><STMTRS><CURDEF>USD</CURDEF><BANKACCTFROM><BANKID>123</BANKID><ACCTID>456</ACCTID><ACCTTYPE>CHECKING</ACCTTYPE></BANKACCTFROM><BANKTRANLIST><DTSTART>20260814</DTSTART><DTEND>20260814</DTEND><STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20260814120000</DTPOSTED><TRNAMT>-42.50</TRNAMT><FITID>20260814001</FITID><NAME>STARBUCKS</NAME><MEMO>Latte</MEMO></STMTTRN></BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";
        let normalised = normalise_sgml(s);
        eprintln!("normalised:\n{normalised}");
        let rows = parse(s);
        eprintln!("parsed {} rows: {:#?}", rows.len(), rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-08-14");
        assert_eq!(rows[0].debit, "42.50");
        assert_eq!(rows[0].credit, "");
        assert_eq!(rows[0].description, "Latte");
        assert_eq!(rows[0].payee.as_deref(), Some("STARBUCKS"));
        assert_eq!(rows[0].reference.as_deref(), Some("20260814001"));
    }

    #[test]
    fn parses_ofx2_xml() {
        let s = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><BANKTRANLIST><STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20260814120000</DTPOSTED><TRNAMT>-42.50</TRNAMT><FITID>20260814001</FITID><NAME>STARBUCKS</NAME><MEMO>Latte</MEMO></STMTTRN></BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";
        let rows = parse(s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-08-14");
        assert_eq!(rows[0].debit, "42.50");
    }

    #[test]
    fn positive_amount_is_credit() {
        let s = "OFXHEADER:100\n<OFX><STMTTRN><DTPOSTED>20260814</DTPOSTED><TRNAMT>100.00</TRNAMT><NAME>Acme</NAME></STMTTRN></OFX>";
        let rows = parse(s);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].credit, "100.00");
        assert_eq!(rows[0].debit, "");
    }

    #[test]
    fn sgml_value_with_space() {
        let line = "MEMO Latte with milk";
        let (tag, value) = parse_sgml_value(line).unwrap();
        assert_eq!(tag, "MEMO");
        assert_eq!(value, "Latte with milk");
    }
}
