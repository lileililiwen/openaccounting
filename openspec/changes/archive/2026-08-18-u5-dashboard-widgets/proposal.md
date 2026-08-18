# Configurable Dashboard Widgets

## Why

The dashboard (`/ledgers/{id}/dashboard`) has fixed widgets. Users
have different priorities; one may want net worth, another budget burn.

## What Changes

- Each user picks which widgets appear and in what order.
- Built-in widgets: net-worth line chart, top 5 expense categories,
  budget vs actual, recent transactions, account balances.
- Layout persisted per user.
- New users get a sensible default layout.

## Capabilities

### New Capabilities

- `dashboard-widgets`: Per-user configurable dashboard.

## Impact

**New files:**
- `migrations/0042_add_dashboard_layout.sql`.
- `src/handlers/dashboard_layout.rs`.
- `tests/http/dashboard_layout.rs`.
