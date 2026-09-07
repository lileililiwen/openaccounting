//! Structured e-invoice serializers (`e-invoicing-facturx`).
//!
//! Two pure serializers over one invoice snapshot:
//!
//! - [`cii_xml`] — Cross-Industry Invoice, EN 16931 COMFORT subset.
//!   This is the payload embedded in Factur-X / ZUGFeRD 2.x PDFs and
//!   the format accepted by XRechnung receivers. Generation is
//!   fallible: mandatory profile fields missing from the ledger data
//!   produce a field-level error list instead of a broken document.
//! - [`ubl_xml`] — UBL 2.1 Invoice, schema-validatable; used where
//!   UBL is the required interchange syntax.
//!
//! PDF/A-3 embedding (the Factur-X wrapper) is a separate concern and
//! is not implemented here.

use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct EinvoiceParty {
    pub name: String,
    pub vat_id: Option<String>,
    pub address_line: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    /// ISO 3166-1 alpha-2.
    pub country_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EinvoiceLine {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct EinvoiceData {
    pub number: String,
    pub date: chrono::NaiveDate,
    pub due_date: chrono::NaiveDate,
    pub currency: String,
    pub seller: EinvoiceParty,
    pub buyer: EinvoiceParty,
    pub lines: Vec<EinvoiceLine>,
    /// Sum of line amounts (net).
    pub total_net: Decimal,
    pub payment_terms: Option<String>,
    pub payment_means_code: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("missing mandatory e-invoice fields: {}", .0.join(", "))]
pub struct MissingFields(pub Vec<String>);

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Fields the EN 16931 COMFORT profile mandates that we cannot infer.
fn validate(d: &EinvoiceData) -> Result<(), MissingFields> {
    let mut missing = Vec::new();
    for (role, party) in [("seller", &d.seller), ("buyer", &d.buyer)] {
        if party.name.trim().is_empty() {
            missing.push(format!("{role}.name"));
        }
        if party
            .vat_id
            .as_deref()
            .map(str::trim)
            .map(str::is_empty)
            .unwrap_or(true)
        {
            missing.push(format!("{role}.vat_id"));
        }
        if party.country_code.is_none() {
            missing.push(format!("{role}.country_code"));
        }
    }
    if d.lines.is_empty() {
        missing.push("lines".to_string());
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(MissingFields(missing))
    }
}

fn party_block(tag: &str, p: &EinvoiceParty) -> String {
    let mut s = format!("<{tag}>");
    s.push_str(&format!("<ram:Name>{}</ram:Name>", esc(&p.name)));
    if let Some(vat) = p.vat_id.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!(
            "<ram:SpecifiedTaxRegistration><ram:ID schemeID=\"VA\">{}</ram:ID></ram:SpecifiedTaxRegistration>",
            esc(vat)
        ));
    }
    s.push_str("<ram:PostalTradeAddress>");
    if let Some(line) = p.address_line.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!("<ram:LineOne>{}</ram:LineOne>", esc(line)));
    }
    if let Some(city) = p.city.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!("<ram:CityName>{}</ram:CityName>", esc(city)));
    }
    if let Some(pc) = p.postal_code.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!("<ram:PostcodeCode>{}</ram:PostcodeCode>", esc(pc)));
    }
    if let Some(cc) = p.country_code.as_deref() {
        s.push_str(&format!("<ram:CountryID>{}</ram:CountryID>", esc(cc)));
    }
    s.push_str("</ram:PostalTradeAddress>");
    s.push_str(&format!("</{tag}>"));
    s
}

/// CII (Cross-Industry Invoice) XML, EN 16931 COMFORT subset.
pub fn cii_xml(d: &EinvoiceData) -> Result<String, MissingFields> {
    validate(d)?;
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    s.push_str(
        "<rsm:CrossIndustryInvoice xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\" \
         xmlns:ram=\"urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100\" \
         xmlns:udt=\"urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100\">",
    );
    s.push_str("<rsm:ExchangedDocumentContext><ram:GuidelineSpecifiedDocumentContextParameter>\
                <ram:ID>urn:cen.eu:en16931:2017</ram:ID></ram:GuidelineSpecifiedDocumentContextParameter>\
                </rsm:ExchangedDocumentContext>");
    s.push_str(&format!(
        "<rsm:ExchangedDocument><ram:ID>{}</ram:ID><ram:TypeCode>380</ram:TypeCode>\
         <ram:IssueDateTime><udt:DateTimeString format=\"102\">{}</udt:DateTimeString></ram:IssueDateTime>\
         </rsm:ExchangedDocument>",
        esc(&d.number),
        d.date.format("%Y%m%d")
    ));
    s.push_str("<rsm:SupplyChainTradeTransaction>");
    for (i, line) in d.lines.iter().enumerate() {
        s.push_str(&format!(
            "<ram:IncludedSupplyChainTradeLineItem>\
             <ram:AssociatedDocumentLineDocument><ram:LineID>{}</ram:LineID></ram:AssociatedDocumentLineDocument>\
             <ram:SpecifiedTradeProduct><ram:Name>{}</ram:Name></ram:SpecifiedTradeProduct>\
             <ram:SpecifiedLineTradeAgreement>\
             <ram:NetPriceProductTradePrice><ram:ChargeAmount>{}</ram:ChargeAmount></ram:NetPriceProductTradePrice>\
             </ram:SpecifiedLineTradeAgreement>\
             <ram:SpecifiedLineTradeDelivery><ram:BilledQuantity unitCode=\"H87\">{}</ram:BilledQuantity></ram:SpecifiedLineTradeDelivery>\
             <ram:SpecifiedLineTradeSettlement><ram:SpecifiedTradeSettlementLineMonetarySummation>\
             <ram:LineTotalAmount>{}</ram:LineTotalAmount></ram:SpecifiedTradeSettlementLineMonetarySummation>\
             </ram:SpecifiedLineTradeSettlement>\
             </ram:IncludedSupplyChainTradeLineItem>",
            i + 1,
            esc(&line.description),
            line.unit_price.normalize(),
            line.quantity.normalize(),
            line.amount.normalize(),
        ));
    }
    s.push_str("<ram:ApplicableHeaderTradeAgreement>");
    s.push_str(&party_block("ram:SellerTradeParty", &d.seller));
    s.push_str(&party_block("ram:BuyerTradeParty", &d.buyer));
    s.push_str("</ram:ApplicableHeaderTradeAgreement>");
    s.push_str(&format!(
        "<ram:ApplicableHeaderTradeDelivery><ram:RequestedDeliveryPeriodDateTime>\
         <udt:DateTimeString format=\"102\">{}</udt:DateTimeString>\
         </ram:RequestedDeliveryPeriodDateTime></ram:ApplicableHeaderTradeDelivery>",
        d.due_date.format("%Y%m%d")
    ));
    s.push_str("<ram:ApplicableHeaderTradeSettlement>");
    s.push_str(&format!(
        "<ram:InvoiceCurrencyCode>{}</ram:InvoiceCurrencyCode>",
        esc(&d.currency)
    ));
    if let Some(code) = d.payment_means_code.as_deref().filter(|c| !c.is_empty()) {
        s.push_str(&format!(
            "<ram:SpecifiedTradeSettlementPaymentMeans><ram:TypeCode>{}</ram:TypeCode></ram:SpecifiedTradeSettlementPaymentMeans>",
            esc(code)
        ));
    }
    s.push_str(&format!(
        "<ram:SpecifiedTradeSettlementHeaderMonetarySummation>\
         <ram:LineTotalAmount>{}</ram:LineTotalAmount>\
         <ram:TaxBasisTotalAmount>{}</ram:TaxBasisTotalAmount>\
         <ram:GrandTotalAmount>{}</ram:GrandTotalAmount>\
         <ram:DuePayableAmount>{}</ram:DuePayableAmount>\
         </ram:SpecifiedTradeSettlementHeaderMonetarySummation>",
        d.total_net.normalize(),
        d.total_net.normalize(),
        d.total_net.normalize(),
        d.total_net.normalize(),
    ));
    s.push_str("</ram:ApplicableHeaderTradeSettlement>");
    s.push_str("</rsm:SupplyChainTradeTransaction>");
    s.push_str("</rsm:CrossIndustryInvoice>");
    Ok(s)
}

/// UBL 2.1 Invoice XML.
pub fn ubl_xml(d: &EinvoiceData) -> Result<String, MissingFields> {
    validate(d)?;
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    s.push_str(
        "<Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\" \
         xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" \
         xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\">",
    );
    s.push_str(&format!(
        "<cbc:CustomizationID>urn:cen.eu:en16931:2017</cbc:CustomizationID>"
    ));
    s.push_str(&format!("<cbc:ID>{}</cbc:ID>", esc(&d.number)));
    s.push_str(&format!(
        "<cbc:IssueDate>{}</cbc:IssueDate>",
        d.date.format("%Y-%m-%d")
    ));
    s.push_str("<cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>");
    s.push_str(&format!(
        "<cbc:DocumentCurrencyCode>{}</cbc:DocumentCurrencyCode>",
        esc(&d.currency)
    ));
    s.push_str(&format!(
        "<cac:AccountingSupplierParty><cac:Party>\
         <cac:PartyName><cbc:Name>{}</cbc:Name></cac:PartyName>\
         <cac:PostalAddress>{}</cac:PostalAddress>\
         <cac:PartyTaxScheme><cbc:CompanyID>{}</cbc:CompanyID>\
         <cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme></cac:PartyTaxScheme>\
         </cac:Party></cac:AccountingSupplierParty>",
        esc(&d.seller.name),
        postal_ubl(&d.seller),
        esc(d.seller.vat_id.as_deref().unwrap_or_default()),
    ));
    s.push_str(&format!(
        "<cac:AccountingCustomerParty><cac:Party>\
         <cac:PartyName><cbc:Name>{}</cbc:Name></cac:PartyName>\
         <cac:PostalAddress>{}</cac:PostalAddress>\
         <cac:PartyTaxScheme><cbc:CompanyID>{}</cbc:CompanyID>\
         <cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme></cac:PartyTaxScheme>\
         </cac:Party></cac:AccountingCustomerParty>",
        esc(&d.buyer.name),
        postal_ubl(&d.buyer),
        esc(d.buyer.vat_id.as_deref().unwrap_or_default()),
    ));
    s.push_str(&format!(
        "<cac:PaymentMeans><cbc:PaymentMeansCode>{}</cbc:PaymentMeansCode></cac:PaymentMeans>",
        esc(d.payment_means_code.as_deref().unwrap_or("30"))
    ));
    for (i, line) in d.lines.iter().enumerate() {
        s.push_str(&format!(
            "<cac:InvoiceLine><cbc:ID>{}</cbc:ID>\
             <cbc:InvoicedQuantity unitCode=\"H87\">{}</cbc:InvoicedQuantity>\
             <cbc:LineExtensionAmount currencyID=\"{}\">{}</cbc:LineExtensionAmount>\
             <cac:Item><cbc:Name>{}</cbc:Name></cac:Item>\
             <cac:Price><cbc:PriceAmount currencyID=\"{}\">{}</cbc:PriceAmount></cac:Price>\
             </cac:InvoiceLine>",
            i + 1,
            line.quantity.normalize(),
            esc(&d.currency),
            line.amount.normalize(),
            esc(&line.description),
            esc(&d.currency),
            line.unit_price.normalize(),
        ));
    }
    s.push_str(&format!(
        "<cac:TaxTotal><cbc:TaxAmount currencyID=\"{}\">0</cbc:TaxAmount></cac:TaxTotal>\
         <cac:LegalMonetaryTotal>\
         <cbc:LineExtensionAmount currencyID=\"{}\">{}</cbc:LineExtensionAmount>\
         <cbc:TaxExclusiveAmount currencyID=\"{}\">{}</cbc:TaxExclusiveAmount>\
         <cbc:PayableAmount currencyID=\"{}\">{}</cbc:PayableAmount>\
         </cac:LegalMonetaryTotal>",
        esc(&d.currency),
        esc(&d.currency),
        d.total_net.normalize(),
        esc(&d.currency),
        d.total_net.normalize(),
        esc(&d.currency),
        d.total_net.normalize(),
    ));
    s.push_str("</Invoice>");
    Ok(s)
}

fn postal_ubl(p: &EinvoiceParty) -> String {
    let mut s = String::new();
    if let Some(line) = p.address_line.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!("<cbc:StreetName>{}</cbc:StreetName>", esc(line)));
    }
    if let Some(city) = p.city.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!("<cbc:CityName>{}</cbc:CityName>", esc(city)));
    }
    if let Some(pc) = p.postal_code.as_deref().filter(|v| !v.is_empty()) {
        s.push_str(&format!("<cbc:PostalZone>{}</cbc:PostalZone>", esc(pc)));
    }
    if let Some(cc) = p.country_code.as_deref() {
        s.push_str(&format!(
            "<cac:Country><cbc:IdentificationCode>{}</cbc:IdentificationCode></cac:Country>",
            esc(cc)
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> EinvoiceData {
        EinvoiceData {
            number: "2026-000001".into(),
            date: chrono::NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            due_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
            currency: "EUR".into(),
            seller: EinvoiceParty {
                name: "Seller GmbH".into(),
                vat_id: Some("DE123456789".into()),
                address_line: Some("Str. 1".into()),
                city: Some("Berlin".into()),
                postal_code: Some("10115".into()),
                country_code: Some("DE".into()),
            },
            buyer: EinvoiceParty {
                name: "Buyer SA".into(),
                vat_id: Some("FR09876543210".into()),
                address_line: None,
                city: None,
                postal_code: None,
                country_code: Some("FR".into()),
            },
            lines: vec![
                EinvoiceLine {
                    description: "Widget".into(),
                    quantity: Decimal::new(2, 0),
                    unit_price: Decimal::new(50, 0),
                    amount: Decimal::new(100, 0),
                },
                EinvoiceLine {
                    description: "Setup fee".into(),
                    quantity: Decimal::new(1, 0),
                    unit_price: Decimal::new(2550, 2),
                    amount: Decimal::new(2550, 2),
                },
            ],
            total_net: Decimal::new(12550, 2),
            payment_terms: Some("Net 30".into()),
            payment_means_code: Some("30".into()),
        }
    }

    #[test]
    fn cii_totals_equal_invoice_totals() {
        let xml = cii_xml(&fixture()).unwrap();
        assert!(
            xml.contains("<ram:DuePayableAmount>125.5</ram:DuePayableAmount>"),
            "{xml}"
        );
        assert!(xml.contains("urn:cen.eu:en16931:2017"));
        assert_eq!(xml.matches("IncludedSupplyChainTradeLineItem").count(), 4); // open+close ×2 tags
    }

    #[test]
    fn ubl_payable_amount_equals_invoice_total() {
        let xml = ubl_xml(&fixture()).unwrap();
        assert!(
            xml.contains("<cbc:PayableAmount currencyID=\"EUR\">125.5</cbc:PayableAmount>"),
            "{xml}"
        );
        assert!(xml.starts_with("<?xml"));
    }

    #[test]
    fn missing_mandatory_fields_block_generation_with_field_list() {
        let mut d = fixture();
        d.seller.vat_id = None;
        d.buyer.country_code = None;
        let err = cii_xml(&d).unwrap_err();
        assert!(err.to_string().contains("seller.vat_id"), "{err}");
        assert!(err.to_string().contains("buyer.country_code"), "{err}");
        assert!(ubl_xml(&d).is_err());
    }

    #[test]
    fn property_random_totals_round_trip() {
        let mut seed = 42u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..50 {
            let cents = (next() % 10_000_00) as i64 + 1;
            let mut d = fixture();
            d.lines = vec![EinvoiceLine {
                description: "x".into(),
                quantity: Decimal::ONE,
                unit_price: Decimal::new(cents, 2),
                amount: Decimal::new(cents, 2),
            }];
            d.total_net = Decimal::new(cents, 2);
            let xml = ubl_xml(&d).unwrap();
            assert!(
                xml.contains(&format!(">{}</cbc:PayableAmount", d.total_net.normalize())),
                "cents={cents}"
            );
        }
    }
}
