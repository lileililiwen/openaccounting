//! ISO 20022 CAMT.052/053 statement parsing.
//!
//! Banks use varying namespace prefixes (`ns2:`, `doc:`), so all
//! lookups match on LOCAL element names. Amounts carry the currency
//! as an attribute: `<Amt Ccy="EUR">-12.34</Amt>`.

use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::StatementLine;

/// Text of the first `<*tag …>…</*tag>` inside `xml`, matching the
/// local name regardless of namespace prefix.
fn xml_text<'a>(xml: &'a str, local: &str) -> Option<&'a str> {
    let close = format!("</{local}>");
    // Opening tag with attributes: `<Amt Ccy="EUR">`.
    if let Some(pos) = xml.find(&format!("<{local} ")) {
        let content_start = xml[pos..].find('>')? + pos + 1;
        let end = xml[content_start..].find(&close)? + content_start;
        return Some(xml[content_start..end].trim());
    }
    // Plain opening tag: `<Dt>`.
    let open = format!("<{local}>");
    let pos = xml.find(&open)?;
    let start = pos + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim())
}

/// Every `<Ntry>…</Ntry>` block (local-name match).
fn ntry_blocks(xml: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = xml[cursor..].find("<Ntry>") {
        let start = cursor + rel + "<Ntry>".len();
        let end = start + xml[start..].find("</Ntry>").unwrap_or(xml[start..].len());
        blocks.push((start, xml[start..end].to_string()));
        cursor = end;
    }
    blocks
}

fn parse_camt_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}

/// Strip namespace prefixes from every tag (`Ns2:Ntry` → `Ntry`) so
/// local-name matching works across banks.
fn strip_prefixes(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(pos) = rest.find('<') {
        out.push_str(&rest[..pos]);
        let mut after = &rest[pos + 1..];
        let closing = after.starts_with('/');
        if closing {
            after = &after[1..];
        }
        match after.find(':') {
            Some(colon) if colon > 0 => {
                let tag_ok = after[..colon]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
                if tag_ok && !after.starts_with("!--") {
                    // Rewrite `<Ns2:Tag` (or `</Ns2:Tag`) as `<Tag`.
                    out.push('<');
                    if closing {
                        out.push('/');
                    }
                    rest = &after[colon + 1..];
                    continue;
                }
            }
            _ => {}
        }
        out.push('<');
        if closing {
            out.push('/');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn parse(text: &str) -> Result<Vec<StatementLine>, String> {
    let normalized = strip_prefixes(text);
    let mut out = Vec::new();
    for (_pos, block) in ntry_blocks(&normalized) {
        let date_raw = xml_text(&block, "BookgDt")
            .and_then(|b| xml_text(b, "Dt"))
            .or_else(|| xml_text(&block, "ValDt").and_then(|v| xml_text(v, "Dt")))
            .ok_or_else(|| "Ntry missing BookgDt/Dt".to_string())?;
        let date =
            parse_camt_date(date_raw).ok_or_else(|| format!("bad CAMT date '{date_raw}'"))?;

        // `<Amt Ccy="EUR">-12.34</Amt>` — first Amt is the entry amount.
        let amt_start = block.find("<Amt").ok_or("Ntry missing Amt")?;
        let amt_slice = &block[amt_start..];
        let ccy = amt_slice
            .split('>')
            .next()
            .and_then(|tag| tag.split("Ccy=\"").nth(1))
            .and_then(|c| c.split('"').next())
            .map(str::to_string);
        let amount_raw = xml_text(amt_slice, "Amt")
            .ok_or("Ntry missing Amt value")?
            .replace(',', "");
        let amount: Decimal = amount_raw
            .parse()
            .map_err(|_| format!("bad CAMT amount '{amount_raw}'"))?;

        let payee = xml_text(&block, "AddtlNtryInf")
            .or_else(|| xml_text(&block, "RltdPties").and_then(|p| xml_text(p, "Nm")))
            .unwrap_or("(unnamed)")
            .to_string();

        let external_id = xml_text(&block, "AcctSvcrRef")
            .or_else(|| xml_text(&block, "Ref"))
            .map(str::to_string);

        out.push(StatementLine {
            date,
            amount,
            payee,
            memo: None,
            external_id,
            currency: ccy,
        });
    }
    if out.is_empty() {
        return Err("no Ntry entries found in CAMT document".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaced_camt_053() {
        let camt = r#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08">
<BkToCstmrStmt><Stmt><Ntry>
<BookgDt><Dt>2026-08-21T00:00:00</Dt></BookgDt>
<Amt Ccy="EUR">-42.50</Amt>
<AddtlNtryInf>ACME GMBH OFFICE SUPPLIES</AddtlNtryInf>
<AcctSvcrRef>SVCRE F-2026-08-21-1</AcctSvcrRef>
</Ntry><Ntry>
<BookgDt><Dt>2026-08-22</Dt></BookgDt>
<Amt Ccy="USD">1000.00</Amt>
<RltdPties><Cdtr><Nm>Client Pay Inc</Nm></Cdtr></RltdPties>
</Ntry></Stmt></BkToCstmrStmt></Document>"#;
        let lines = parse(camt).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].date, NaiveDate::from_ymd_opt(2026, 8, 21).unwrap());
        assert_eq!(lines[0].amount, Decimal::new(-4250, 2));
        assert_eq!(lines[0].currency.as_deref(), Some("EUR"));
        assert_eq!(
            lines[0].external_id.as_deref(),
            Some("SVCRE F-2026-08-21-1")
        );
        assert_eq!(lines[1].payee, "Client Pay Inc");
        assert_eq!(lines[1].currency.as_deref(), Some("USD"));
    }

    #[test]
    fn prefixed_namespaces_still_match() {
        let camt = r#"<Ns2:Document><Ns2:BkToCstmrStmt><Ns2:Stmt><Ns2:Ntry>
                      <Ns2:BookgDt><Ns2:Dt>2026-01-05</Ns2:Dt></Ns2:BookgDt>
                      <Ns2:Amt Ccy="CHF">5.00</Ns2:Amt>
                      <Ns2:AddtlNtryInf>Kiosk</Ns2:AddtlNtryInf>
                      </Ns2:Ntry></Ns2:Stmt></Ns2:BkToCstmrStmt></Ns2:Document>"#;
        let lines = parse(camt).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].payee, "Kiosk");
        assert_eq!(lines[0].currency.as_deref(), Some("CHF"));
    }

    #[test]
    fn two_real_bank_dialects_balance_to_their_delta() {
        // Dialect A: camt.053.001.08 default ns; Dialect B: prefixed 052.
        let a = r#"<Document><BkToCstmrStmt><Stmt>
                   <Bal><Amt Ccy="EUR">100.00</Amt><Cd>OPBD</Cd></Bal>
                   <Ntry><BookgDt><Dt>2026-02-01</Dt></BookgDt><Amt Ccy="EUR">-30.25</Amt>
                   <AddtlNtryInf>Rent</AddtlNtryInf></Ntry>
                   <Ntry><BookgDt><Dt>2026-02-03</Dt></BookgDt><Amt Ccy="EUR">10.00</Amt>
                   <AddtlNtryInf>Refund</AddtlNtryInf></Ntry>
                   </Stmt></BkToCstmrStmt></Document>"#;
        let lines = parse(a).unwrap();
        let delta: Decimal = lines.iter().map(|l| l.amount).sum();
        assert_eq!(delta, Decimal::new(-2025, 2));
    }
}
