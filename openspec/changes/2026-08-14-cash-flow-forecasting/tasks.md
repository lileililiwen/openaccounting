# Cash-Flow Forecasting — Tasks

## 1. Testing

- [ ] 1.1 Unit: `advance("2026-01-31", monthly, 1)` =
      `2026-02-28`.
- [ ] 1.2 Unit: `advance("2026-02-28", monthly, 1)` =
      `2026-03-28`.
- [ ] 1.3 Unit: `advance("2026-08-14", weekly, 1)` =
      `2026-08-21`.
- [ ] 1.4 Unit: `materialize_forecast` produces N entries for
      monthly + interval 1 over 30 days.
- [ ] 1.5 Unit: `project` running balance matches
      hand-computed scenario.
- [ ] 1.6 Integration: `http_forecast_renders_chart` — assert
      SVG path string.
- [ ] 1.7 Integration:
      `http_forecast_footer_summary_correct`.
- [ ] 1.8 Integration:
      `http_forecast_commit_all_writes_entries`.
- [ ] 1.9 Property:
      `prop_projection_balance_is_monotonic_in_openings` for
      1000 random runs.

## 2. Implementation

- [ ] 2.1 `src/reports/cash_flow_forecast.rs` —
      `materialize_forecast`, `project`, `advance`.
- [ ] 2.2 `src/charts/mod.rs` — `forecast_line_chart` SVG
      helper.
- [ ] 2.3 `src/handlers/cash_flow_forecast.rs` — `show`,
      `commit_all`.
- [ ] 2.4 `src/templates/cash_flow_forecast.rs` Askama
      struct.
- [ ] 2.5 `templates/reports/cash_flow_forecast.html`.
- [ ] 2.6 `src/main.rs` — 2 new routes.
- [ ] 2.7 `src/reports/mod.rs` — `pub mod cash_flow_forecast;`.
- [ ] 2.8 Update `templates/partials/_nav.html` "Reports"
      menu — add "Cash flow → Forecast".

## 3. Validation

- [ ] 3.1 `openspec validate cash-flow-forecasting` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: create a recurring "Rent" rule
      (monthly 3000), request forecast 90d, assert two drops
      on the 1st of each month.
- [ ] 3.6 `openspec archive cash-flow-forecasting`.
