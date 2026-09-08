# Tasks

## 1. Testing

- [x] 1.1 Define canonical synthetic ledgers for core bookkeeping, closing/reversal, documents, and audit-chain behavior.
- [x] 1.2 Define exact expected outputs for all core reports using `Decimal` values.
- [x] 1.3 Add fixtures for tax, FX, invoices/AR/AP, amortization, inventory, imports, exports, and idempotency.
- [x] 1.4 Add boundary cases for rounding, zero values, negative values, dates, currencies, and period close.
- [x] 1.5 Add executable checks that report-review metadata and non-compliance disclaimers exist for advanced workflows.

## 2. Implementation

- [x] 2.1 Add the canonical fixture format and loader under the existing test-support conventions.
- [x] 2.2 Add report reconciliation assertions for the fixture outputs.
- [x] 2.3 Add import/export round-trip assertions with documented lossy-field handling.
- [x] 2.4 Add accounting-treatment records and reviewer ownership for each high-consequence workflow.
- [x] 2.5 Update user-facing documentation with supported jurisdiction/scope and compliance disclaimers.

## 3. Verification

- [x] 3.1 Run all reference fixtures against a fresh database.
- [x] 3.2 Run every supported import/export round trip twice and verify idempotency.
- [x] 3.3 Block production-readiness claims for workflows with unresolved treatment or review status.
