# Document Inbox & Multi-Leg Entry

## Why

Real-world feedback on `a12-transaction-entry-ease`:

1. **Transactions are not two-leg.** A real transaction often has
   many accounts on the debit side and many on the credit side
   (split expenses, multiple payment methods). The two-leg "Simple"
   default is too narrow.
2. **Documents are attached two ways, and both must exist:**
   - **Way 1 — inline:** on the transaction new form (already built
     in a12) and on the transaction edit flow, so the document is
     attached at the moment the entry is created or corrected.
   - **Way 2 — separate:** an upload form that stores a document
     without a transaction, and a bind form that attaches that
     stored document to an already-existing transaction (the
     bookkeeper's "gather receipts, then attach" workflow).

## What Changes

- The new-transaction form defaults to the **multi-leg** balancing-line
  editor; the two-leg Simple view stays as an optional shortcut.
- A new ledger-level upload endpoint (`POST /ledgers/{id}/documents`)
  stores an **unbound** document (no transaction).
- The ledger Documents list becomes an **inbox**: unbound documents
  are listed with a "bind to transaction" action.
- A **bind** flow attaches an unbound document to a transaction
  (chosen by date/amount/description search, or created fresh) and is
  audit-logged.
- Inline attach on the new form is retained (a12); the transaction
  show page already offers upload for an existing transaction.

## Capabilities

### New Capabilities

- `document-inbox`: upload documents without a transaction and bind
  them to a transaction later.
- `transaction-multileg-default`: the multi-leg editor is the default
  entry view.

## Impact

**New files:**

- `migrations/0047_add_unbound_documents.sql`.
- `templates/documents/bind.html` (bind form).

**Modified files:**

- `src/domain/document.rs` — `transaction_id` becomes `Option<Uuid>`,
  add `ledger_id: Option<Uuid>`.
- `src/handlers/documents.rs` — ledger-level upload, inbox list,
  bind action, authorization for unbound documents.
- `src/handlers/transactions.rs` + `templates/transactions/new.html`
  — multi-leg editor as the default view.
- `static/js/entry.js` — default mode flips to Advanced.
- `tests/integration/document_authorization.rs`,
  `tests/integration/transactions_document.rs` — new tests.
