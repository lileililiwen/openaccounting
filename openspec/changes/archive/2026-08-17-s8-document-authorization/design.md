# ## Context

The ledger-sharing capability (`migrations/0008_add_ledger_sharing.sql`)
defines owner / editor / viewer roles for ledgers. Transactions, accounts,
and reports enforce these roles via `ensure_owner` / `ensure_editor` /
`ensure_access`. Documents do not.

## Goals / Non-Goals

**Goals:**
- Consistency with other resources.

**Non-Goals:**
- Per-document ACL (over-engineering; ledger-level is enough).

## Decisions

- 404 not 403: hiding existence is the right default for cross-tenant
  boundaries.
- One helper `ensure_doc_access(user, doc_id, required_role)` that does
  the join and the role check.

## Risks / Trade-offs

- One extra SQL query per document download. Acceptable; documents are not
  on the hot path.
