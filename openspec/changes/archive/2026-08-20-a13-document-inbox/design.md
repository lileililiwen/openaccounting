# Document Inbox & Multi-Leg Entry

## Context

`a12` added a two-leg "Simple" default and an inline document attach
on the create form. Real transactions are frequently multi-leg, and
practitioners also attach documents *after* the fact from a separate
upload. This change makes the multi-leg editor the default and adds a
second, additive document path: upload-unbound → bind-to-transaction
(the capture → match → attach pattern used by Dext/Hubdoc, where the
image is attached to an existing transaction and never republishes
data).

## Goals / Non-Goals

**Goals:**

- Multi-leg entry as the default; Simple remains an optional shortcut.
- A ledger-level upload endpoint that stores an unbound document.
- An inbox view of unbound documents and a bind action to an existing
  transaction, audit-logged.
- Keep the a12 inline attach on the create form and the show-page
  upload for existing transactions.

**Non-Goals:**

- OCR / data extraction from documents (documents remain proof only).
- Automatic match suggestions (amount/date heuristics) beyond manual
  search — a future change.
- A rendered transaction edit form (the app edits by reversal +
  correction today; document changes go through the show page).

## Decisions

- **`documents.transaction_id` becomes nullable, and `documents`
  gains a nullable `ledger_id`.** Unbound documents belong to a
  ledger (for listing, authorization, and the inbox UI). Bound
  documents keep their transaction's ledger; the column is set at
  upload and used as the authorization anchor for unbound files.
  **Why:** documents have no ledger column today — they inherit it
  from the transaction. Unbound documents need an anchor.
- **Authorization model:** uploading and binding require writer
  access to the ledger. Download of a bound document keeps the
  existing member rule. Download of an unbound document is restricted
  to the ledger's writers (no transaction to gate on). Non-members
  get 404 for both.
- **`Document.transaction_id`/`ledger_id` become `Option<Uuid>` in
  the domain struct.** This is the invasive part: every query that
  decodes `Document` must still work. **Mitigation:** keep columns
  selected by name; the handful of sites that consume
  `doc.transaction_id` branch on `None`.
- **Bind flow is two-step on one page:** from the inbox, "Bind" opens
  a search form (date/amount/description) plus a "create a new
  transaction" link; choosing a transaction sets `transaction_id`.
- **Multi-leg default is a one-line flip in `entry.js`** (initial
  mode = advanced) plus the template's default `hidden` states.

## Risks / Trade-offs

- Making `transaction_id` nullable could break code that assumes a
  document is always bound. **Mitigation:** audit all `Document`
  consumers; the download path already derives from `stored_filename`;
  the authorization checks are the main ones to branch on.
- The documents list joins through transactions today; unbound rows
  need a `ledger_id`-based query branch. **Mitigation:** a
  `WHERE t.ledger_id = $1 OR d.ledger_id = $1` shape with the join.
- Legacy rows have `ledger_id NULL`. **Mitigation:** the bound-branch
  join derives the ledger from the transaction, so legacy rows still
  resolve.
