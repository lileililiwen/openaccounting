# ## Context

No print CSS.

## Goals / Non-Goals

**Goals:**
- Letter/A4-ready output.

**Non-Goals:**
- PDF export (separate change — covered by o1 if we add PDF rendering).

## Decisions

- One `@media print` block in app.css; per-page overrides only when
  needed.
