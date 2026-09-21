//! SAF-T lite XML export for accountants' handoff
//! (`compliance-exports`).
//!
//! This is a **lite** subset: master data (chart of accounts)
//! plus GL entries for a chosen period. Full country-specific
//! SAF-T (audit file variants, tax tables, customer master
//! records, etc.) varies per jurisdiction and is out of scope.
//! Spec: see `openspec/changes/compliance-exports/design.md`.

use crate::export::LedgerSnapshot;

/// Render SAF-T lite XML for the snapshot, limited to postings in
/// `[from, to]`. The output validates against the checked-in
/// fixture in `tests/fixtures/saf_t_lite_minimal.xml`.
pub fn render(snap: &LedgerSnapshot, from: chrono::NaiveDate, to: chrono::NaiveDate) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<AuditFile xmlns=\"urn:OpenAccounting:SAF-Tlite\">\n");
    out.push_str("  <Header>\n");
    out.push_str(&format!(
        "    <CompanyName>{}</CompanyName>\n",
        xml_escape(&snap.ledger.name)
    ));
    out.push_str(&format!(
        "    <CurrencyCode>{}</CurrencyCode>\n",
        xml_escape(&snap.ledger.base_currency)
    ));
    out.push_str(&format!("    <FromDate>{}</FromDate>\n", from));
    out.push_str(&format!("    <ToDate>{}</ToDate>\n", to));
    out.push_str("  </Header>\n");
    out.push_str("  <ChartOfAccounts>\n");
    for a in &snap.accounts {
        out.push_str("    <Account>\n");
        out.push_str(&format!("      <Id>{}</Id>\n", a.id));
        out.push_str(&format!("      <Name>{}</Name>\n", xml_escape(&a.name)));
        out.push_str(&format!("      <Type>{}</Type>\n", xml_escape(&a.r#type)));
        out.push_str(&format!(
            "      <Currency>{}</Currency>\n",
            xml_escape(&a.currency)
        ));
        out.push_str("    </Account>\n");
    }
    out.push_str("  </ChartOfAccounts>\n");
    out.push_str("  <GeneralLedgerEntries>\n");
    for txn in &snap.transactions {
        if txn.txn_date < from || txn.txn_date > to {
            continue;
        }
        out.push_str("    <Transaction>\n");
        out.push_str(&format!("      <Id>{}</Id>\n", txn.id));
        out.push_str(&format!("      <Date>{}</Date>\n", txn.txn_date));
        out.push_str(&format!(
            "      <Description>{}</Description>\n",
            xml_escape(&txn.description)
        ));
        out.push_str("      <Postings>\n");
        for p in snap.postings.iter().filter(|p| p.transaction_id == txn.id) {
            out.push_str("        <Posting>\n");
            out.push_str(&format!(
                "          <AccountId>{}</AccountId>\n",
                p.account_id
            ));
            out.push_str(&format!(
                "          <Direction>{}</Direction>\n",
                p.direction
            ));
            out.push_str(&format!("          <Amount>{}</Amount>\n", p.amount));
            out.push_str("        </Posting>\n");
        }
        out.push_str("      </Postings>\n");
        out.push_str("    </Transaction>\n");
    }
    out.push_str("  </GeneralLedgerEntries>\n");
    out.push_str("</AuditFile>\n");
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
    fn xml_well_formed_with_ampersand_escaped() {
        let snap = LedgerSnapshot {
            ledger: crate::export::LedgerMeta {
                id: uuid::Uuid::nil(),
                owner_id: uuid::Uuid::nil(),
                name: "A & B".into(),
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
        assert!(xml.contains("A &amp; B"));
        assert!(xml.contains("<?xml"));
        assert!(xml.contains("</AuditFile>"));
    }
}
