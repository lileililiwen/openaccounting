# Reconciliation Automation — Design

## Schema

```sql
-- migrations/0021_add_reconciliation_rules.sql
CREATE TABLE reconciliation_rules (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id     UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL
        CHECK (kind IN ('match','categorize','flag')),
    priority      INT NOT NULL DEFAULT 100,
    predicate     JSONB NOT NULL DEFAULT '{}',
    action        JSONB NOT NULL DEFAULT '{}',
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reconciliation_rules_ledger_priority_idx
    ON reconciliation_rules (ledger_id, priority, is_active);
```

## Predicate evaluator

```rust
// src/domain/reconciliation_rules.rs
pub fn evaluate_predicate(
    p: &serde_json::Value,
    line: &ImportedLine,
) -> bool {
    macro_rules! get_str { ($k:literal) => { p.get($k).and_then(|v| v.as_str()) }; }
    macro_rules! get_i64 { ($k:literal) => { p.get($k).and_then(|v| v.as_i64()) }; }

    if let Some(glob) = get_str!("payee_glob") {
        if !sql_like_match(glob, &line.payee) { return false; }
    }
    if let Some(glob) = get_str!("description_glob") {
        if !sql_like_match(glob, &line.description) { return false; }
    }
    if let Some(eq) = get_i64!("amount_cents_eq") {
        if line.amount_cents != eq { return false; }
    }
    if let Some(lt) = get_i64!("amount_cents_lt") {
        if line.amount_cents >= lt { return false; }
    }
    if let Some(gt) = get_i64!("amount_cents_gt") {
        if line.amount_cents <= gt { return false; }
    }
    if let Some(off) = get_i64!("date_offset_days_eq") {
        let expected = (Utc::now() + Duration::days(off)).date_naive();
        if line.txn_date != expected { return false; }
    }
    if let Some(ccy) = get_str!("currency") {
        if line.currency != ccy { return false; }
    }
    true
}

fn sql_like_match(pattern: &str, s: &str) -> bool {
    // naive but adequate: % and _ wildcards
    let regex = "^".to_string()
        + &pattern.replace('%', ".*").replace('_', ".")
        + "$";
    regex::Regex::new(&regex).map(|r| r.is_match(s)).unwrap_or(false)
}
```

## Apply action

`POST /ledgers/{id}/rules/{rule_id}/apply` is the same
commit handler as
`2026-08-14-csv-import-completion`, parameterized by the
rule's action JSON. Categorize → create transaction with
DR/CR; Match → set `imported_line.posting_id =
target_posting.id` and mark the line reconciled; Flag →
set `imported_line.flag_reason` and `flag_color`.

## UI changes

`templates/reconciliation/page.html` renders each un-reconciled
line with a column "Suggestions": a list of badges (one per
matching rule, ordered by priority). Each badge is a button
that POSTs `/rules/{id}/apply` with the line id; success
swaps the row to a reconciled state.

## Tests

### Unit

- `evaluate_predicate` true for each single-key predicate on
  a matching line, false on a non-matching line.
- Compound predicates are AND-combined.
- `sql_like_match` handles `%` and `_` wildcards.
- Priority tiebreaking returns the lowest-numbered matching
  rule.

### Integration

- `http_categorize_rule_suggestion_renders` — rule + line +
  GET reconciliation → badge "Suggested: Travel & Meals".
- `http_apply_categorize_creates_transaction` — POST
  `/apply` → 303, 1 new transaction in DB.
- `http_match_rule_pairs_lines` — POST `/apply` → 0 new
  txns, line marked reconciled.
- `http_priority_tiebreak_picks_lower` — two rules, lower
  priority wins.
- `http_rule_create_validates_predicate_types` — invalid
  JSON shape → 400.

### Property

- `prop_predicate_is_dnf_of_atoms` — any compound predicate
  is equivalent to the conjunction of its atoms; verified on
  1000 random (predicate, line) pairs.

## References

- Firefly III rules engine
- hledger CSV rules (`finance` package pattern)
- GnuCash import-match assistant
