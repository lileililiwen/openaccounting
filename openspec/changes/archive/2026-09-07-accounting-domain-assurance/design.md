# Design

## Decisions

### Use executable reference fixtures

Synthetic ledger fixtures will describe transactions, postings, currencies, tax rates, dates, documents, and expected balances/reports. Tests will compare normalized outputs rather than fragile HTML. Fixtures will cover both ordinary and boundary cases.

### Review accounting treatment explicitly

Each advanced workflow will have a short decision record identifying debit/credit treatment, recognition date, rounding policy, reversal/void behavior, and report inclusion. A qualified reviewer can approve or reject the treatment without reading all handler code.

### Validate round trips

Beancount, hledger, CSV, JSON, and bank-statement imports/exports will be checked for lossless or explicitly documented lossy round trips. Duplicate detection and idempotency will be part of the fixture suite.

### Risks and mitigations

- A synthetic fixture can encode an incorrect accounting assumption; mitigate with named external review and approval records.
- Cross-jurisdiction tax behavior varies; mitigate by labeling jurisdiction scope and keeping generic engine behavior separate from policy configuration.
- Decimal/FX edge cases can create small discrepancies; mitigate with exact `Decimal` expected values and explicit rounding assertions.

## Verification

Run the reference fixture suite against fresh databases, compare reports and exports to expected values, run import idempotency tests, and record domain-review status for every advanced workflow before calling the feature production-ready.
