# Enforce Per-Document Authorization

## Why

Document download (`src/lib.rs:215-221`) currently takes a `Path<doc_id>`
with no ledger/share check. Any logged-in user can fetch any document by ID,
bypassing the `ensure_owner` pattern that protects transactions, accounts,
etc. This is a horizontal privilege escalation.

## What Changes

- `documents::download` MUST call a `ensure_doc_access(user, doc_id)`
  helper that joins `documents → transactions → ledgers` and checks either
  ownership or membership in `ledger_members`.
- Same check applies to `documents::delete` and `documents::ocr`.
- Returns 404 (not 403) to avoid revealing existence of documents outside
  the user's scope.

## Capabilities

### New Capabilities

- `document-authorization`: Per-document ledger/share authorization.

## Impact

**Modified files:**
- `src/handlers/documents.rs` — add `ensure_doc_access`.
- `src/handlers/document_ocr.rs` — same check.
- `tests/http/documents.rs` — new cases.
