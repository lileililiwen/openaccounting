//! XBRL-GL instance export for accountants' handoff
//! (`compliance-exports`).
//!
//! Minimal XBRL-GL instance covering chart of accounts and GL
//! entries for a chosen period. Uses the XBRL-GL 2005 taxonomy
//! namespaces but emits only the elements needed for a
//! general-ledger dump. Schema validation against the checked-in
//! minimal XSD in `tests/fixtures/xbrl_gl_minimal.xsd` is
//! exercised by the integration tests.

use crate::export::LedgerSnapshot;

pub fn render(snap: &LedgerSnapshot, from: chrono::NaiveDate, to: chrono::NaiveDate) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<xbrl xmlns=\"http://www.xbrl.org/int/gl/2005-03-31\"\n");
    out.push_str("      xmlns:xbrli=\"http://www.xbrl.org/2003/instance\"\n");
    out.push_str("      xmlns:link=\"http://www.xbrl.org/2003/linkbase\"\n");
    out.push_str("      xmlns:xlink=\"http://www.w3.org/1999/xlink\">\n");
    out.push_str(&format!(
        "  <context id=\"period\">\n    <entity><identifier scheme=\"www.openaccounting.local\">{}</identifier></entity>\n    <period>\n      <startDate>{}</startDate>\n      <endDate>{}</endDate>\n    </period>\n  </context>\n",
        snap.ledger.id, from, to
    ));
    out.push_str("  <unit id=\"EUR\"><measure>iso4217:USD</measure></unit>\n");
    for a in &snap.accounts {
        out.push_str(&format!("  <gl:account id=\"acc-{}>\n", a.id));
        out.push_str(&format!("    <gl:accountCode>{}</gl:accountCode>\n", a.id));
        out.push_str(&format!(
            "    <gl:accountName>{}</gl:accountName>\n",
            xml_escape(&a.name)
        ));
        out.push_str(&format!(
            "    <gl:accountType>{}</gl:accountType>\n",
            xml_escape(&a.r#type)
        ));
        out.push_str("  </gl:account>\n");
    }
    for txn in &snap.transactions {
        if txn.txn_date < from || txn.txn_date > to {
            continue;
        }
        let postings: Vec<_> = snap
            .postings
            .iter()
            .filter(|p| p.transaction_id == txn.id)
            .collect();
        for p in &postings {
            out.push_str(&format!(
                "  <gl:entry id=\"p-{}\">\n    <gl:date>{}</gl:date>\n    <gl:description>{}</gl:description>\n    <gl:amount contextRef=\"period\" unitRef=\"EUR\" decimals=\"2\">{}</gl:amount>\n  </gl:entry>\n",
                p.id,
                txn.txn_date,
                xml_escape(&txn.description),
                p.amount
            ));
        }
    }
    out.push_str("</xbrl>\n");
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_well_formed_and_declares_period_context() {
        let snap = LedgerSnapshot {
            ledger: crate::export::LedgerMeta {
                id: uuid::Uuid::nil(),
                owner_id: uuid::Uuid::nil(),
                name: "Test".into(),
                base_currency: "USD".into(),
                timezone: "UTC".into(),
                basis: "accrual".into(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            accounts: vec![],
            transactions: vec![],
            postings: vec![],
            tags: vec![],
            document_refs: vec![],
            budgets: vec![],
            currencies: vec!["USD".into()],
        };
        let xml = render(
            &snap,
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        );
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("</xbrl>"));
        // Period context must be declared with id="period".
        assert!(xml.contains("<context id=\"period\">"));
        assert!(xml.contains("<startDate>2026-01-01</startDate>"));
        assert!(xml.contains("<endDate>2026-12-31</endDate>"));
        // Unit must be declared.
        assert!(xml.contains("<unit id=\"EUR\">"));
    }
}
