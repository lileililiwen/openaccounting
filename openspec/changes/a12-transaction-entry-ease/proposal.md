# Easier Transaction Entry

## Why

Recording a transaction is the most frequent action in the app, and
today it demands double-entry thinking: the user picks two accounts
and types two amounts that must balance by hand. The form already
auto-balances the *last* line, but that behaviour is implicit and
fragile, there is no "simple" mode for a plain expense/income, and
the source document — the receipt or invoice that proves the entry —
is attached on a *separate* screen after saving.

Real bookkeepers work document-first (gather the receipt, then
record) and attach the image to the transaction as the proof. The
entry should let them do both in one place, with the balance computed
for them.

## What Changes

- A first-class **balancing line**: one posting is always auto-
  computed (amount + direction) so Σ debits = Σ credits, with a clear
  status readout near the save button.
- A **simple entry mode** (default) that builds the two legs from
  everyday language — "spent 5.50 on Office Supplies from Bank
  Account" — instead of exposing Debit/Credit rows up front.
- **Inline document attach**: upload the receipt/invoice on the same
  form; the create request saves the transaction and links the
  document to it in one step.
- The advanced multi-line editor is retained (splits, transfers) and
  gains the balancing-line behaviour.

## Capabilities

### New Capabilities

- `transaction-balancing-line`: one auto-computed posting line.
- `transaction-simple-entry`: everyday-language two-leg entry.
- `transaction-balance-status`: live "ready / needs X" readout.
- `transaction-document-inline`: attach documents while entering.

## Impact

**New files:**

- `templates/partials/_posting_editor.html` (shared editor: simple +
  advanced views).
- `static/js/entry.js` (balancing-line + simple-mode + submit builder).

**Modified files:**

- `templates/transactions/new.html` — editor swap + inline upload.
- `static/js/split.js` — folded into the balancing-line model.
- `src/handlers/transactions.rs` — create handler accepts multipart
  with optional document files.
- `src/handlers/documents.rs` / storage — reuse the existing upload
  path for the inline files.
- `tests/integration/transactions_split.rs`, `transactions_draft.rs`,
  `document_authorization.rs` — new HTTP tests.
