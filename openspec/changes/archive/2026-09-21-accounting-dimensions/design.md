## Context

Postings carry account/amount/direction with optional foreign amounts and tax base. Tags exist at transaction level. Recurrence exists for invoices (`recurring_invoices.rs`) and amortization schedules. Inventory and fixed-asset handlers exist with treatment docs but single-method assumptions. FX manual rates beat feed rows with no who/why record.

## Goals / Non-Goals

**Goals:**
- Month-end close runs from templates with preview, not hand-typed journals.
- Every valuation choice is disclosed on the reports that depend on it.

**Non-Goals:**
- Full job-costing or manufacturing BOMs (dimensions cover analysis, not production).
- Payroll engine (still deferred; dimensions do not imply employees).

## Decisions

- **Dimensions as nullable `cost_center_id`/`project_id` on postings with ledger-scoped dimension tables.** WHY: posting-level granularity supports split allocations within one transaction; NULL keeps existing data valid.
- **Recurring journals as templates generating draft transactions for preview, posted by explicit confirm or scheduler.** WHY: preview-before-post prevents silent mis-postings; drafts reuse the existing draft flow.
- **Inventory method per ledger set at creation, changeable only with zero stock or via audited restatement.** WHY: mid-stream method switches restate history; the guard forces an explicit decision.
- **FX overrides append to an `fx_override_audit` log; lookup precedence unchanged.** WHY: preserves multi-currency-fx behavior while adding the missing who/why.
- **Depreciation choice per asset (straight-line default, declining-balance optional) with method stored on the asset.** WHY: asset-level choice matches small-business reality better than a global switch.

## Risks / Trade-offs

- Dimension slices slow large TB queries → Mitigation: composite indexes on (ledger_id, cost_center_id, project_id, date).
- Recurring scheduler double-post → Mitigation: idempotency key per template plus period; unique constraint on (template_id, period).
- FIFO layers complicate adjustments → Mitigation: adjustments consume newest layer first and are disclosed; property tests cover layer math.
