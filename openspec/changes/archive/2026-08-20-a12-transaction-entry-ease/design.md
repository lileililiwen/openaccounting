# Easier Transaction Entry

## Context

Recording a transaction is the daily core of the app. Research on how
professional bookkeepers actually work shapes this change:

1. **Documents come first.** The canonical daily workflow is: gather
   source documents (receipts, invoices, bank statements) → record the
   transactions → categorize → reconcile
   ([ABusinessManager, *Daily Bookkeeping Tasks Explained*, 2026-06](https://abusinessmanager.com/2026/06/daily-bookkeeping-tasks-explained-what-bookkeepers-do/)).
   The same source names "failing to attach receipts or provide
   adequate descriptions" as a top integrity mistake. The document is
   the primary evidence, captured alongside the entry.
2. **Attach is a capture → match → attach pipeline.** Capture tools
   attach document images to existing transactions by matching
   supplier + amount + date; they send *only the image*, never
   republishing the financial data — the transaction stays the source
   of truth, the image is the proof
   ([Dext help centre, *Paperwork Match*, 2026-02](https://help.dext.com/en/articles/377069-how-to-use-paperwork-match-with-your-accounting-software)).
   When no match exists, the bookkeeper creates the transaction and
   attaches manually.

OpenAccounting already stores documents as attachments to
transactions (`documents.transaction_id`) and renders an upload on the
transaction show page. The gap is the *entry* flow: it demands manual
balancing and defers documents to a second screen.

## Goals / Non-Goals

**Goals:**

- The balance of a transaction is computed for the user, never typed
  twice.
- A plain expense/income/transfer can be recorded from everyday
  language without Debit/Credit rows.
- The receipt can be attached in the same step as creating the
  transaction.
- The advanced editor stays for splits and transfers.

**Non-Goals:**

- OCR / data extraction from uploaded documents (the document never
  *drives* the entry; it is proof). Post-save OCR is a separate change
  (`s9`/`d6` OCR feedback exists; extraction itself is out of scope).
- Bank-feed / "find & match" reconciliation — a future change.

## Decisions

- **The balancing line is a JS concept, not a backend concept.** The
  form always submits the standard `lines[N][…]` structure; the server
  keeps its existing balance validation. This keeps the backend
  unchanged for the editor work and guarantees the invariant.
- **One line is *the* balancing line.** It is pinned by the user or
  inferred (last empty line, else the last line). Editing any other
  line recomputes its amount; its direction flips when the sign of the
  remainder flips. Editing the balancer itself promotes the next empty
  line. **Why:** the existing "last row" heuristic is implicit and
  breaks as soon as a middle line is edited; an explicit, movable
  balancer is predictable.
- **Simple mode builds the legs client-side.** "Expense 5.50 / Office
  Supplies / Bank Account" maps to `DEBIT Office Supplies 5.50` +
  `CREDIT Bank Account 5.50`. The simple fields are never sent as
  postings; JS injects hidden `lines[N]` inputs on submit. **Why:** the
  server's `parse_lines` stays untouched, and Advanced mode keeps all
  existing behaviour.
- **Inline documents ride the create request as multipart.** The
  create handler currently parses urlencoded form data; it gains a
  multipart variant that also writes the uploaded files through the
  existing storage path and links them to the new transaction. **Why:**
  document-first entry (upload before save) matches the bookkeeper
  workflow; the post-create show page already covers attach-after.
- **Document ≠ entry.** Uploaded images are attachments only; no
  category/amount is inferred from them (per the Dext finding that the
  image is attached without republishing data).

## Risks / Trade-offs

- Making the create handler accept multipart touches CSRF and the
  request-body limit. **Mitigation:** keep the urlencoded path working
  (feature-flag by content-type), reuse the existing multipart
  document-upload handler code, and test both paths.
- An auto-balancing line can surprise users who edit it. **Mitigation:**
  the balancer is visually marked ("auto"); typing into it converts it
  to a fixed line, so nothing is silently overwritten.
- Simple mode hides Debit/Credit, which power users may want. **Mitigation:**
  a persistent Simple/Advanced toggle that preserves entered data.
