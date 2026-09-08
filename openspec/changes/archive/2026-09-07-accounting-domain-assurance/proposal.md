# Accounting domain assurance

## Why

The project has expanded from core double-entry bookkeeping into tax, invoices, AR/AP, FX, bank feeds, amortization, inventory, e-invoicing, and multi-entity workflows. These are high-consequence financial features. The repository has implementation and tests, but no explicit reference dataset, accounting-treatment review, or release gate proving that reports and imports agree with an authoritative expected result.

## What changes

- Add capability `accounting-assurance`.
- Define canonical synthetic ledgers and expected journal/report outputs for core and advanced workflows.
- Add domain review records for accounting treatment, rounding, dates, currencies, taxes, reversals, closing, and audit behavior.
- Add acceptance gates for imports/exports and report reconciliation.
- Document that passing software tests is not equivalent to jurisdiction-specific tax or accounting compliance.

## Non-goals

- No claim of legal, tax, or regulatory certification.
- No automatic selection of jurisdiction-specific tax rules.
- No redesign of the accounting data model in this change.
