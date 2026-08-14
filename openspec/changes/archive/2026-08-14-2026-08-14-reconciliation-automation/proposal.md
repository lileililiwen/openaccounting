# Add Reconciliation Automation

## Why

The current reconciliation page (`src/handlers/reconciliation.rs`)
lets a user manually tick off imported lines against existing
ledger postings. There is no automatic matching, no rule-based
auto-categorization, and no policy to enforce consistent account
selection on similar transactions. For any non-trivial ledger
this is hours of tedium per month.

Firefly III ships "rules" with `description matches` and
`amount equals` triggers. GnuCash's import-match assistant and
hledger's CSV rules are the inspiration for this change.

## What Changes

- New capability `reconciliation-rules` with three rule kinds:
  **match** (auto-pair imported line ↔ existing posting),
  **categorize** (auto-set GL account on imported lines),
  **flag** (highlight anomalies for manual review).
- New table `reconciliation_rules`
  (`id, ledger_id, kind, priority, predicate_json,
   action_json, is_active, created_at`).
- A rule's predicate can reference any of:
  `amount_cents`, `txn_date ± N days`, `payee_glob` (SQL
  `LIKE` pattern), `description_glob`, `currency`.
- A rule's action can be:
  for **match**: `link_to_posting_id` or `link_by_amount_date`;
  for **categorize**: `gl_account_id`;
  for **flag**: `reason_text` and a UI highlight color.
- New routes:
  - `GET  /ledgers/{id}/rules`
  - `GET  /ledgers/{id}/rules/new`
  - `POST /ledgers/{id}/rules`
  - `POST /ledgers/{id}/rules/{rule_id}/toggle`
  - `POST /ledgers/{id}/rules/{rule_id}/delete`
- The reconciliation page runs active rules against un-reconciled
  imported lines and displays auto-match / auto-categorize
  suggestions; the user accepts each suggestion with one click.

## Capabilities

### New Capabilities

- `reconciliation-rules` — rule-based auto-match and
  auto-categorize.

## Impact

- **New files:**
  - `migrations/0021_add_reconciliation_rules.sql`
  - `src/domain/reconciliation_rules.rs`
  - `src/handlers/rules.rs`
  - `src/templates/rules.rs`
  - `templates/rules/{list,new}.html`
  - `tests/integration/reconciliation_rules.rs`
  - `tests/fixtures/reconciliation_rules/`
- **Modified files:**
  - `src/main.rs` — 5 new routes.
  - `src/handlers/reconciliation.rs` — apply rules on page load.
  - `templates/partials/_nav.html` — add "Rules" link.
  - `templates/reconciliation/page.html` — show suggested
    matches with accept buttons.

## Non-Goals

- ML-based categorization (LLM calls etc.). Pattern matching
  only.
- Cross-ledger rules.
- Rule versioning / audit-of-rule-edits (separate change).
