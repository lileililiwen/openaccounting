# Cash-Basis Toggle — Design

## Schema

```sql
-- migrations/0020_add_ledger_basis.sql
ALTER TABLE ledgers
    ADD COLUMN basis TEXT NOT NULL DEFAULT 'accrual'
    CHECK (basis IN ('accrual','cash'));
```

Existing rows are backfilled automatically by the `DEFAULT`.

## Domain type

```rust
// src/reports/mod.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ReportBasis { Accrual, Cash }

impl Default for ReportBasis { fn default() -> Self { Self::Accrual } }

impl ReportBasis {
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "accrual" | "" => Ok(Self::Accrual),
            "cash" => Ok(Self::Cash),
            other => Err(AppError::Validation(format!(
                "Unknown basis '{}', expected 'accrual' or 'cash'.", other
            ))),
        }
    }
}
```

## Income-statement SQL (cash-basis variant)

Existing query (accrual) sums all INCOME/EXPENSE postings in the
period. Cash-basis variant adds a `WHERE` filter that excludes any
posting whose sibling-leg account is AR or AP:

```sql
-- src/reports/income_statement.rs (cash-basis)
SELECT a.id, a.name, a.type,
       SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END) AS dr,
       SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END) AS cr
FROM postings p
JOIN accounts a ON a.id = p.account_id
JOIN transactions t ON t.id = p.transaction_id
WHERE a.ledger_id = $1
  AND t.txn_date BETWEEN $2 AND $3
  AND a.type IN ('INCOME','EXPENSE')
  AND NOT EXISTS (
      SELECT 1 FROM postings p2
      JOIN accounts a2 ON a2.id = p2.account_id
      WHERE p2.transaction_id = p.transaction_id
        AND p2.id <> p.id
        AND a2.subtype IN ('accounts_receivable','accounts_payable')
  )
GROUP BY a.id, a.name, a.type
ORDER BY a.type, a.name;
```

The excluded-amounts footer line (the "Revenue excluded" footnote
in the scenario) is computed by the same query minus the `NOT
EXISTS` filter, then subtracted.

## UI

`templates/reports/income_statement.html` (and the cash-flow
sibling) gain a small form near the date inputs:

```html
<div class="inline-flex rounded-md border border-slate-300 overflow-hidden text-sm">
  <a href="?from={{from}}&to={{to}}&basis=accrual"
     class="px-3 py-1 {{ if basis == Accrual }}bg-slate-900 text-white{{ else }}bg-white{{ endif }}">
     Accrual
  </a>
  <a href="?from={{from}}&to={{to}}&basis=cash"
     class="px-3 py-1 {{ if basis == Cash }}bg-slate-900 text-white{{ else }}bg-white{{ endif }}">
     Cash
  </a>
</div>
```

## Tests

### Unit

- `ReportBasis::parse` accepts `"accrual"`, `"cash"`, and empty
  string; rejects everything else.

### Integration

- `http_income_statement_accrual_includes_ar_revenue` — assert
  revenue = 1000 on accrual.
- `http_income_statement_cash_excludes_ar_revenue` — assert
  revenue = 0 and the excluded-amounts footer = 1000 on cash.
- `http_income_statement_basis_invalid_returns_400` — assert 400
  on `?basis=foo`.
- `http_ledger_create_with_cash_basis_persists` — assert column
  set.
- `http_cash_flow_footer_shows_basis` — assert footer text.

## References

- README §"Features" — "Cash-basis single-entry" and
  "double-entry can, automatically"
- hledger `--cash` / `cashflow` flag (similar concept)
- GnuCash "Use cash basis accounting" option
