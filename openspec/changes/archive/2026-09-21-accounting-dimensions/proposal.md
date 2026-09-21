# Proposal: Dimensions, recurring journals, and valuation depth

## Why

The engine posts balanced transactions well, but professional books need analysis dimensions (cost centers, projects), recurring journals (rent, depreciation, subscriptions — not just invoices or amortization), explicit inventory valuation methods, and auditable manual FX overrides. Today there are plain tags plus saved searches, invoice-only recurrence, single-method inventory/assets, and ECB-plus-manual FX without an override audit. Month-end work stays manual and error-prone.

## What Changes

- Posting dimensions: cost-center and project fields on postings with dimension-aware trial balance and P&L slices.
- Recurring journal templates with schedule, end date, skip/pause, and preview-before-post.
- Inventory valuation choice per ledger (FIFO vs weighted-average) with method disclosure on reports.
- Manual FX rate override audit (who/when/old/new) and revaluation that respects overrides.
- Straight-line vs declining-balance depreciation choice for fixed assets with disposal gain/loss posting.

## Capabilities

### New Capabilities
- `accounting-dimensions`: dimensions, recurring journals, valuation methods, FX override audit, depreciation choices.

### Modified Capabilities
- `multi-currency-fx`: manual rates gain an override audit trail without changing lookup precedence.
- `bookkeeping`: postings MAY carry dimensions without relaxing the balance invariant.
- `reports`: TB/P&L gain dimension slices without changing core math.

## Impact

Affected: `src/domain/posting.rs`, posting service, `src/reports/trial_balance.rs` and `income_statement.rs`, inventory/asset handlers, FX handlers, new `recurring_journals` tables and worker. Unaffected: auth, bank feeds, PWA, notifications.
