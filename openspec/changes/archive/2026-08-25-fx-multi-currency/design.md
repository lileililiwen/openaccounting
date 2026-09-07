# Design — fx-multi-currency

## Context

Currency columns exist everywhere but no rate data. The posting
service (`src/domain/`, posting-service spec) is the single write path
for postings, which makes it the only place foreign amounts need to be
derived.

## Goals / Non-Goals

**Goals:**
- Base-currency books stay provably balanced (A = L + E unchanged).
- Dated, auditable rates; manual overrides win.

**Non-Goals:**
- Live intraday rates or paid feed vendors (ECB daily + manual covers
  SMB bookkeeping).
- Multi-commodity accounting (GnuCash-style trading accounts).

## Decisions

- **Store rates as pair rows, not a pivot table.** WHY: pairs are what
  lookups need; ECB publishes EUR-base crosses that we store verbatim
  and invert on lookup when needed (inverse computed once per request,
  never stored). Alternative considered: full NxN materialized matrix —
  rejected because it doubles write paths for no read benefit.
- **Derive base amount at save time and freeze it on the posting.**
  WHY: reports stay simple SQL sums over base amounts; later rate edits
  never silently restate history (matches append-only-mode spec).
  Alternative considered: convert at render time like Firefly III's
  optional mode — rejected: restates closed periods.
- **Revaluation posts real transactions** rather than a parallel
  valuation ledger. WHY: keeps the audit chain and trial balance
  authoritative; Xero does the same with month-end journals.

## Risks / Trade-offs

- Rounding drift between foreign legs → Mitigation: derive every leg
  from its own foreign amount, then assert the DB trigger still holds;
  reject the transaction otherwise.
- ECB feed outage → Mitigation: worker is idempotent and backfills on
  next success; manual entry always available.
- New dependency `rustybucek/ecb`-style crate avoided → Mitigation:
  fetch the plain CSV/JSON endpoint with `reqwest` (already a
  dependency); no new crate.

## Migration Plan

1. Add `fx_rates` + nullable `foreign_amount`/`foreign_currency` on
   `postings` (nullable = zero existing behavior changes).
2. Wire derivation into posting service behind an always-on code path.
3. Add report + revaluation routes.
