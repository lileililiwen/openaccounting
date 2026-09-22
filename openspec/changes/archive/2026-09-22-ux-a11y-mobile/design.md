## Context

Server-rendered HTMX pages swap partials without full reloads. Charts are hand-written SVG. Six locale JSON bundles exist with English fallback plus warning logs. Capacitor config points at a server URL with push-token registration.

## Goals / Non-Goals

**Goals:**
- Keyboard-only users can post a transaction, reconcile, and read every report.
- Missing translations are visible before release, not after.

**Non-Goals:**
- Full offline bookkeeping (decision is receipt-draft queue at most).
- Native design-system rebuild (web UI stays the surface).

## Decisions

- **Audit first against WCAG 2.2 AA with `axe-core` on rendered fixtures plus manual screen-reader pass.** WHY: automated plus manual is the minimum credible evidence; fixtures make it repeatable in CI.
- **HTMX swaps move focus to the updated region heading and announce via `aria-live`.** WHY: without this, screen-reader users never learn a partial updated; heading focus is the standard pattern.
- **Every chart ships a visually-hidden data table plus `role=img` with `aria-label` summary.** WHY: SVG paths are unreadable to assistive tech; tables preserve the data with zero new endpoints.
- **Locale gate: per-language missing-key percentage published; release blocked above 5% for supported languages.** WHY: hard-zero blocks community languages; a threshold keeps the promise honest.
- **Mobile decision defaults to supported-Capacitor only if receipt capture lands; otherwise retire `mobile/` and document PWA install.** WHY: a shell without a capture flow adds maintenance for no user value. Alternative considered: full native rewrite — rejected: team size and single-binary strategy forbid it.

## Risks / Trade-offs

- Axe on server HTML misses HTMX dynamic states → Mitigation: run axe after representative swaps in E2E harness, not just initial HTML.
- Translation threshold churn → Mitigation: report is informational for community languages, blocking only for the six day-1 locales.
- Capacitor store review burden → Mitigation: decision gates this; no store submission until capture flow passes acceptance.
