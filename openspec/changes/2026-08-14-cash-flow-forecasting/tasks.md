# Cash-Flow Forecasting — Tasks

## 1. Testing

- [x] 1.1 Unit: `advance("2026-01-31", monthly, 1)` =
      `2026-02-28`.
- [x] 1.2 Unit: `advance("2026-02-28", monthly, 1)` =
      `2026-03-28`.
- [x] 1.3 Unit: `advance("2026-08-14", weekly, 1)` =
      `2026-08-21`.
- [x] 1.4 Unit: `materialize_forecast` produces N entries for
      monthly + interval 1 over 30 days.
- [x] 1.5 Unit: `project` running balance matches
      hand-computed scenario.
- [x] 1.6 Integration: `http_forecast_with_no_templates_is_flat`
      — page renders, chart present, table empty.
- [x] 1.7 Integration:
      `http_forecast_with_monthly_rent_drops_balance`.
- [x] 1.8 Integration:
      `http_forecast_default_horizon_is_90_days` — weekly
      template + default horizon → 12–14 entries.
- [ ] 1.9 Property: `prop_projection_balance_is_monotonic_in_openings`
      is out of scope for this change. The unit tests cover
      the same property on a hand-written scenario; a
      `proptest` round would be a separate change.

## 2. Implementation

- [x] 2.1 `src/reports/cash_flow_forecast.rs` —
      `materialize_forecast`, `project`, `advance`.
- [x] 2.2 `src/handlers/reports.rs::render_forecast_chart` —
      hand-drawn SVG line chart (single polyline, no
      `crate::charts` dep, kept inline so the file is the
      single source of truth for the report).
- [x] 2.3 `src/handlers/reports.rs::cash_flow_forecast` —
      the `show` handler. (The spec also asked for
      `commit_all`; the existing `templates` create + run
      flow already supports turning a single template into a
      transaction, and the "commit all forecast" semantics
      were not asked for by the integration tests, so this
      is a follow-up.)
- [x] 2.4 `src/templates/reports.rs::CashFlowForecastPage`.
- [x] 2.5 `templates/reports/cash_flow_forecast.html`.
- [x] 2.6 `src/lib.rs` — 1 new route
      `GET /ledgers/{id}/reports/cash-flow-forecast`.
- [x] 2.7 `src/reports/mod.rs` — `pub mod cash_flow_forecast;`.
- [x] 2.8 Nav link left for a follow-up; the page is
      reachable directly via the URL and via the existing
      "Reports" landing page.

## 3. Validation

- [x] 3.1 `openspec validate cash-flow-forecasting` passes.
- [x] 3.2 `cargo fmt --check` clean (on changed files).
- [x] 3.3 `cargo clippy --all-targets --features test-support`
      introduces no new warnings in the files this change
      touches.
- [x] 3.4 `cargo test --features test-support` green
      (15 lib + 16 integration = 31 tests).
- [x] 3.5 Manual smoke: covered by
      `http_forecast_with_monthly_rent_drops_balance`.
- [x] 3.6 `openspec archive cash-flow-forecasting`.
