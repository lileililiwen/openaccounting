## Context

Reports are thin SQL-backed Askama pages (~80–380 lines each). Exports fan out through `src/handlers/export.rs` and `src/export/`. Invoice printing is HTML-only; the Factur-X XML path exists but is marked experimental.

## Goals / Non-Goals

**Goals:**
- Byte-stable archivable PDFs reproducible from the same ledger snapshot.
- Machine exports accountants can open in DATEV and audit tools.

**Non-Goals:**
- Certified Peppol access point (still out of scope; Factur-X file correctness only).
- Jurisdiction tax-rule packs (rates stay generic; formats are jurisdiction-agnostic).

## Decisions

- **PDF via `printpdf`/`genpdf`-style pure-Rust generation, not headless Chromium.** WHY: keeps the single-binary promise with no system browser dependency. Alternative considered: headless Chromium render — rejected: 100MB+ runtime dep and nondeterministic output.
- **SAF-T lite subset (master data + GL entries) first, full audit-file later.** WHY: subset covers the accountant handoff; full SAF-T varies per country and would stall the change.
- **Comparatives computed by re-running the same report queries with shifted date params.** WHY: no new math paths; guarantees current and prior columns agree by construction.
- **Drill-down as query links (report line → filtered GL), not new endpoints.** WHY: reuses existing GL filtering; minimal new surface.
- **Notes stored per (ledger, period) as plain text with author + timestamp.** WHY: disclosures need attribution without a full document workflow.

## Risks / Trade-offs

- PDF layout drift across versions → Mitigation: golden-file hash tests on a fixture ledger block layout regressions.
- XBRL-GL schema strictness → Mitigation: validate against a checked-in XSD in tests; emit minimal valid instance first.
- New PDF crate weight → Mitigation: gate behind default-on but swappable module; document binary size delta in PR.
