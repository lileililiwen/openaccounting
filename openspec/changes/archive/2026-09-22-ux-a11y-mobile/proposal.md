# Proposal: Accessibility, locale coverage, and mobile clarity

## Why

The UI has dark mode, keyboard shortcuts, empty states, dashboard widgets, and six locales with fallback logging — but no WCAG audit evidence, no focus management for HTMX partials, no screen-reader treatment for hand-rolled SVG charts, and no locale-coverage report. Meanwhile `mobile/` ships a Capacitor shell while README non-goals say "responsive web UI only", and PWA has a manifest without an offline story. Users cannot tell what mobile is promised.

## What Changes

- WCAG 2.2 AA audit with remediation across forms, HTMX partials, charts, and printable views.
- Focus management, live-region announcements, chart text alternatives, and keyboard-path E2E for core flows.
- Locale coverage gate: missing-key report per language failing CI above threshold.
- Explicit mobile decision: Capacitor shell becomes supported with receipt-capture flow, or is removed and README/PWA promise responsive-web-only with installable PWA.

## Capabilities

### New Capabilities
- `ux-a11y`: audit evidence, focus/live-region behavior, chart alternatives, locale coverage gate, mobile promise decision.

### Modified Capabilities
- `localization`: gains a coverage report and threshold without changing locale resolution order.
- `keyboard-shortcuts`: gains chart/partial focus paths without changing existing bindings.
- `dark-mode`: contrast fixes land here without changing theme selection.
- `pwa`: offline scope is decided explicitly (receipt-draft queue or documented no-offline).
- `mobile`: shell is either supported with capture flow or retired.

## Impact

Affected: Askama templates, `src/charts/`, HTMX partials, `static/locales/*`, `mobile/`, PWA manifest/service worker scope. Unaffected: accounting math, API shapes, backup/restore.
