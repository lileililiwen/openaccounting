# Cash-Flow Forecasting — Design

## Materializing recurring transactions

The existing `recurring_transactions` table (migration
`0009_add_recurring_transactions.sql`) holds rows like:

```sql
freq TEXT  -- 'daily' | 'weekly' | 'monthly' | 'yearly'
interval INT  -- every N freq units
next_run_date DATE
template_postings JSONB  -- e.g. [{"account":"...","direction":"DR","amount":"3000"}]
```

A new function `materialize_forecast(pool, ledger_id, days)`
walks each active row and emits one synthetic `ParsedRow`
per occurrence in `[today, today+days]`.

```rust
pub fn materialize_forecast(
    pool: &PgPool,
    ledger_id: Uuid,
    days: i64,
) -> AppResult<Vec<ForecastEntry>> {
    let rules = sqlx::query_as::<_, RecurringRule>(
        "SELECT * FROM recurring_transactions
         WHERE ledger_id = $1 AND is_active = TRUE"
    ).bind(ledger_id).fetch_all(pool).await?;

    let today = Utc::now().date_naive();
    let horizon = today + Duration::days(days);
    let mut out = vec![];
    for r in rules {
        let mut next = r.next_run_date;
        while next <= horizon {
            out.push(ForecastEntry {
                date: next,
                amount: r.template_amount,
                description: r.description.clone(),
                payee: r.payee.clone(),
                category: r.category.clone(),
            });
            next = advance(next, r.freq, r.interval);
        }
    }
    Ok(out)
}
```

`advance(date, freq, interval)` adds the right delta:

- `daily`: `+ interval days`
- `weekly`: `+ interval weeks`
- `monthly`: `+ interval months` (clamp day-of-month to last
  day if needed — e.g. Jan 31 + 1 month = Feb 28/29)
- `yearly`: `+ interval years`

We use `chrono::Months::new` / `chrono::Days::new` to avoid
manual month arithmetic bugs.

## Projection

```rust
pub fn project(
    opening_balance: Decimal,
    entries: &[ForecastEntry],
    today: NaiveDate,
) -> Vec<(NaiveDate, Decimal)> {
    let mut running = opening_balance;
    let mut by_date: BTreeMap<NaiveDate, Decimal> = BTreeMap::new();
    for e in entries {
        *by_date.entry(e.date).or_default() += e.amount;  // signed
    }
    let mut out = vec![(today, opening_balance)];
    for (date, delta) in by_date {
        running += delta;
        out.push((date, running));
    }
    out.sort_by_key(|(d, _)| *d);
    out
}
```

## SVG chart

`src/charts/mod.rs` gains `forecast_line_chart(points)` that
renders the same look as the existing line chart.

## Commit-all

`POST /ledgers/{id}/reports/cash-flow-forecast/commit`
re-materializes the entries (so we don't trust the
client-rendered list), inserts the corresponding
transactions, and advances each recurring rule's
`next_run_date`.

## Tests

### Unit

- `advance("2026-01-31", monthly, 1)` = `2026-02-28`.
- `advance("2026-02-28", monthly, 1)` = `2026-03-28`.
- `advance("2026-08-14", weekly, 1)` = `2026-08-21`.
- `materialize_forecast` produces N entries for a row with
  monthly + interval 1 over 30 days.
- `project` running balance matches a hand-computed scenario.

### Integration

- `http_forecast_renders_chart` — assert SVG contains
  expected line path.
- `http_forecast_footer_summary_correct` — assert
  `min_balance`, `max_balance`, `ending_balance`.
- `http_forecast_commit_all_writes_entries` — 3 entries
  committed, `next_run_date` advanced.

### Property

- `prop_projection_balance_is_monotonic_in_openings` —
  varying opening balance over 1000 random runs produces
  monotonic projections.

## References

- hledger `--forecast` flag
- Beancount `forecast` extension
- GnuCash "Scheduled Transactions" + "Future Balance" report
