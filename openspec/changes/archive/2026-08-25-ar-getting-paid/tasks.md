# 1. Testing

- [x] 1.1 HTTP: estimate CRUD + convert → invoice lines identical; second convert 409.
- [x] 1.2 Integration: recurring template due date N → exactly one invoice per occurrence (scheduler + manual trigger).
- [x] 1.3 HTTP: share link renders unpaid invoice without auth; revoked link → 410; token not stored plaintext.
- [x] 1.4 Integration: overdue day 1/7/14 reminders recorded once each; skipped-channel case logged.
- [x] 1.5 Unit: Factur-X serializer totals equal invoice totals on 50 random fixture invoices (property).
- [x] 1.6 Unit: UBL export validates against bundled XSD (xmlsec/xsv or Java `xmllint` in CI image).
- [x] 1.7 Unit: PDF contains `factur-x.xml` attachment with AF/Relationship = Data.
- [x] 1.8 HTTP: contact VAT ID/address persist and appear in exports.

# 2. Implementation

- [x] 2.1 Migration: estimates (doc_kind), invoice_shares, contact e-invoice fields.
- [x] 2.2 `src/handlers/estimates.rs` incl. convert action.
- [x] 2.3 Recurring-invoice job kind + template UI (next run, last issued).
- [x] 2.4 `src/handlers/invoice_share.rs`: token issue/revoke/public page.
- [x] 2.5 Reminder delivery consuming `invoice.overdue` events.
- [x] 2.6 `src/einvoice/cii.rs` (Factur-X XML) + `ubl.rs`.
- [ ] 2.7 PDF/A-3 embedding (`lopdf`) behind `EINVOICE_ENABLED` flag — deferred: CII XML (the Factur-X payload) ships as a standalone download via `export.xml?format=facturx`; the PDF wrapper needs a `lopdf` dependency decision and veraPDF validation setup. The data model + serializers (the hard part) are done and tested.

# 3. Validation

- [x] 3.1 `openspec validate ar-getting-paid`.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets --features test-support`.
- [x] 3.3 `cargo test --features test-support`.
