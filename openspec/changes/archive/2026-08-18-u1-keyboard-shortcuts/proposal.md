# Keyboard Shortcuts

## Why

GnuCash has `Ctrl+T` for transfer, etc. OpenAccounting has none.
Power users expect them.

## What Changes

- Global `g` then `l`/`t`/`a`/`r` to jump to Ledgers / Transactions /
  Accounts / Reports.
- `c` to create a new transaction when on the list page.
- `?` to show a help overlay.
- All bindings listed in the help overlay; no overlap with browser
  defaults.

## Capabilities

### New Capabilities

- `keyboard-shortcuts`: Keyboard navigation.

## Impact

**New files:**
- `static/js/shortcuts.js`.
- `templates/partials/_shortcuts_help.html`.
