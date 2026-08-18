## 1. Testing

- [x] 1.1 HTTP: `http_dashboard_default_layout`.
- [x] 1.2 HTTP: `http_dashboard_add_widget`.
- [x] 1.3 HTTP: `http_dashboard_reorder`.
- [x] 1.4 HTTP: `http_dashboard_reset_default`.
- [x] 1.5 HTTP: `http_dashboard_rejects_unknown_widget` (added; spec asked for "must offer Reset" + persistence; unknown IDs must not silently corrupt the row).

## 2. Implementation

- [x] 2.1 `migrations/0034_add_dashboard_layouts.sql`.
- [x] 2.2 `src/handlers/dashboard_layout.rs` — `load` / `save` / `reset`, `set_layout` + `reset_layout` handlers, plus the SQL helpers `budget_burn_rows` and `account_balances_rows` that feed the two new widgets.
- [x] 2.3 `templates/dashboard/_picker.html` — embedded directly into `templates/dashboard.html` (a separate partial was unnecessary because the picker is a one-liner above the widget list).

Also added:
- Two new widgets, `budget_burn` (per-budget spent-vs-amount progress bars) and `account_balances` (top-10 by absolute balance).
- The dashboard template now iterates `layout` and emits one `<section data-widget-id="...">` per ID in the user's order.
- The picker form submits a comma-separated `widgets` field; this sidesteps a serde_urlencoded limitation where repeated keys collapse to the last value when deserialising into a `Vec`.

## 3. Validation

- [x] 3.1 `openspec validate u5-dashboard-widgets`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u5-dashboard-widgets`.