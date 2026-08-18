# Printable CSS for Reports

## Why

Reports currently look good on screen but print awkwardly — the
sidebar nav, the dark header, and the row spacing make paper output
useless.

## What Changes

- A `@media print` CSS block hides nav, header, footer; widens
  tables; uses serif font for the report body.
- A `Print` button on each report page triggers `window.print()`.
- Page break rules: avoid breaking inside a table row; insert page
  break before major sections.

## Capabilities

### New Capabilities

- `printable-views`: Print-friendly CSS.

## Impact

**Modified files:**
- `static/css/app.css` — `@media print`.
- `templates/base.html` — print-button slot per page.
- `templates/reports/*.html` — explicit print hooks.
