# e-invoicing-facturx Specification (delta)

## ADDED Requirements

### Requirement: Factur-X Embedded PDF

The printable invoice view SHALL offer "Download e-invoice (PDF)"
producing a PDF/A-3 with embedded `factur-x.xml` (CII, profile
EN 16931 / COMFORT) plus the `AFRelationship` metadata. Required
fields (seller, buyer, line net/tax/category, totals, payment
reference) SHALL come from the invoice + ledger + contact records;
missing mandatory fields SHALL block generation with a field-level
error list.

#### Scenario: Valid embedded XML

- **WHEN** a complete invoice is exported
- **THEN** the PDF contains `factur-x.xml` whose computed totals equal
  the invoice totals to the cent.

#### Scenario: Incomplete seller data blocks export

- **WHEN** the ledger lacks a VAT ID required by the chosen profile
- **THEN** generation fails listing exactly which fields are missing.

### Requirement: UBL Export

Each invoice SHALL be exportable as UBL 2.1 (`Invoice/cac:…`)
via `GET /invoices/{id}/export.xml?format=ubl`, schema-validating
against the bundled XSD in tests.

#### Scenario: Round-trip validation

- **WHEN** any fixture invoice is exported as UBL
- **THEN** it validates against the XSD and its `cbc:PayableAmount`
  equals the invoice total.

### Requirement: Data Model Readiness

The invoice/contact model SHALL carry the fields needed by EN 16931:
contact legal name, postal address, VAT/tax ID, payment terms, and
payment means code — surfaced in the contact edit form; legacy rows
simply leave them blank.

#### Scenario: New fields persist

- **WHEN** a contact is saved with VAT ID and address
- **THEN** exports include them without further mapping.
