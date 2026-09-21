# OpenAccounting roadmap

OpenAccounting is currently v0.1-alpha. The repository contains a working
Rust/PostgreSQL application and an active implementation queue. This roadmap
records delivery order; OpenSpec proposals and accepted specs remain the
normative contracts.

## Current state

- Implemented foundations include double-entry posting, authentication,
  documents, reports, imports/exports, audit chain, backups, API routes, and
  responsive web UI.
- The active queue below is specified but not runtime-complete. An OpenSpec
  task count of `0/N` is planning evidence, not implementation evidence.
- The Capacitor mobile shell exists, but its support promise is intentionally
  unresolved until `ux-a11y-mobile` decides whether it is supported or retired.
- Jurisdiction-specific tax compliance, multi-tenant SaaS, formal SOC 2/ISO
  certification, and automatic bank reconciliation remain out of scope.

## Delivery order

### Now — foundation and release contract

1. **`release-readiness`** — roadmap, v1.0 exit gates, governance, docs
   hygiene, operator guidance, and changelog discipline.
2. **`ops-hardening`** — disclosure process, backup targets and restore
   evidence, RTO/RPO, redacted tracing, rate limits, and release attestations.

### Next — accounting control depth

3. **`statement-reconciliation`** — statement sessions, cleared lines,
   zero-difference close, lock/unreconcile, CAMT.053 and QBO imports.
4. **`accounting-dimensions`** — dimensions, allocation, reporting filters,
   and import/export behavior without weakening posting balance.
5. **`pro-close-controls`** — hard close, audited reopen, maker-checker,
   accountant/auditor roles, and gapless invoice numbering.

### Later — integration and user-facing expansion

6. **`openapi-sdk`** — OpenAPI coverage, idempotency, incoming events,
   automation rules, and event catalog.
7. **`compliance-exports`** — PDF/schema exports, comparative reporting,
   report notes, drill-down, and supported e-invoice packaging.
8. **`ux-a11y-mobile`** — WCAG 2.2 AA remediation, focus/live regions,
   chart alternatives, locale coverage, and an explicit mobile decision.

Changes are dependency-ordered for implementation, but they are independent
OpenSpec packages. Complete one package, archive it, produce the required two
commits, then re-read this roadmap and `HANDOFF.md`.

## v1.0 exit gates

The project should not call itself v1.0 until the applicable gates have
evidence in the repository or CI:

- strict OpenSpec validation and zero `TBD - created by archiving` purposes;
- clean format, clippy, unit/integration/property/HTTP tests and documented
  migration rollback evidence;
- security disclosure contact, dependency/security checks, rate-limit policy,
  backup restore drill, and stated RTO/RPO;
- hard-close periods with audited reopen and maker-checker role separation,
  covered by tests;
- statement-reconciliation sessions with a zero-difference close gate;
- server-rendered PDF/A archiving for invoices and core reports;
- documented `/api/v1` contract with idempotency behavior;
- accessibility audit with remediation and an explicit mobile support promise;
- reproducible, signed release artifacts with provenance/SBOM evidence.

## Deferred / non-goals

Native mobile accounting features, multi-tenant SaaS hosting, jurisdiction-
specific tax rule engines, automatic bank-feed matching beyond explicit
helpers, and formal compliance certification require separate proposals.
