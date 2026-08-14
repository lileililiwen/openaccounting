# Policy Engine — Design

## Schema

```sql
-- migrations/0026_add_reimbursement_policies.sql
CREATE TABLE reimbursement_policies (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL
        CHECK (kind IN ('category_cap','receipt_required','per_diem')),
    config      JSONB NOT NULL DEFAULT '{}',
    severity    TEXT NOT NULL DEFAULT 'hard'
        CHECK (severity IN ('hard','soft')),
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reimbursement_policies_ledger_active_idx
    ON reimbursement_policies (ledger_id, is_active, kind);
```

## Evaluator

```rust
// src/domain/policies.rs
pub enum Violation {
    CategoryCapOver { category: String, max: Decimal,
                      actual: Decimal, date: NaiveDate },
    ReceiptMissing { line_id: Uuid, min_amount: Decimal,
                     actual: Decimal },
    PerDiemOver { destination: String, daily_rate: Decimal,
                  actual: Decimal, line_id: Uuid },
}

pub fn evaluate(
    policies: &[Policy],
    lines: &[ReimbursementLine],
    new_line: Option<&ReimbursementLine>,
) -> Vec<Violation> {
    let mut out = vec![];
    for p in policies.iter().filter(|p| p.is_active) {
        match p.kind {
            PolicyKind::CategoryCap => {
                let cfg = CategoryCapCfg::from_json(&p.config);
                let mut by_day: HashMap<NaiveDate, Decimal> = HashMap::new();
                for l in lines.iter().chain(new_line) {
                    if l.category == cfg.category {
                        *by_day.entry(l.txn_date).or_default() += l.amount;
                    }
                }
                for (date, actual) in by_day {
                    if actual > cfg.max_per_day {
                        out.push(Violation::CategoryCapOver {
                            category: cfg.category.clone(),
                            max: cfg.max_per_day,
                            actual, date,
                        });
                    }
                }
            }
            PolicyKind::ReceiptRequired => {
                let cfg = ReceiptRequiredCfg::from_json(&p.config);
                for l in lines.iter().chain(new_line) {
                    if l.amount >= cfg.min_amount
                        && l.receipt_document_id.is_none()
                    {
                        out.push(Violation::ReceiptMissing {
                            line_id: l.id,
                            min_amount: cfg.min_amount,
                            actual: l.amount,
                        });
                    }
                }
            }
            PolicyKind::PerDiem => {
                let cfg = PerDiemCfg::from_json(&p.config);
                // sort lodging lines; group consecutive by claim
                // (handled by caller); for each pair compute
                // nights and check.
                // ...
            }
        }
    }
    out
}
```

## Integration with submit / approve

```rust
// src/handlers/reimbursement.rs
pub async fn submit(...) -> AppResult<Response> {
    let lines = load_lines(...).await?;
    let policies = load_active_policies(...).await?;
    let violations = policies::evaluate(&policies, &lines, None);
    let hard: Vec<_> = violations.iter()
        .filter(|v| severity_is_hard(v, &policies))
        .collect();
    if !hard.is_empty() {
        return Err(AppError::Validation(format!(
            "Policy violation(s): {}", format_violations(&hard)
        )));
    }
    // soft violations → flash message in the 303 redirect
    // transition to submitted
    // ...
}
```

## Per-diem nights computation

For lodging lines in a single claim, sort by `txn_date`.
Group consecutive dates (gap ≤ 1 day). For each group, the
first line gets `nights = group.len() - 1` and the last
`nights = 0` (or 1 for a single-night stay depending on
business rule — we'll use "each line is one night" for
simplicity in v1).

## Tests

### Unit

- `evaluate` returns a `CategoryCapOver` violation when the
  daily sum exceeds the cap.
- `evaluate` returns a `ReceiptMissing` violation when
  `amount >= min_amount` and `receipt_document_id` is None.
- Per-diem over-by-X calculation is correct.

### Integration

- `http_cap_blocks_submit` — add lines totalling over cap →
  400 on submit.
- `http_receipt_required_blocks_approve` — submit a clean
  claim, activate a receipt policy, approve → 400.
- `http_policy_audit_row_written` — creating a policy writes
  an audit row.
- `http_soft_warning_does_not_block` — soft-severity cap
  violation allows submit and surfaces the warning in the
  flash message.

### Property

- `prop_evaluator_is_deterministic` — same inputs → same
  violations for 1000 random claim/policy combos.

## References

- Zoho Expense "Policies"
- SAP Concur policy enforcement
- Rydoo rules engine
