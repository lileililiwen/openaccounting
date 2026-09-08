# Specification and product documentation alignment

## Why

The repository has a large archived OpenSpec history, but current specifications still contain stale `TBD` purposes and describe architecture that no longer matches the source. The README simultaneously advertises bank feeds, invoices, API functionality, and audit capabilities while listing them as v1 non-goals. This weakens contributor onboarding and makes future implementation decisions unsafe.

## What changes

- Add capability `project-governance`.
- Reconcile README features, non-goals, architecture, quick start, and release instructions with the current source.
- Replace stale OpenSpec purpose placeholders with durable capability summaries.
- Correct architecture statements for the library/binary split and dependency versions.
- Add a lightweight rule for keeping archived specs, source, and user-facing documentation aligned.

## Non-goals

- No product-scope expansion.
- No rewriting of historical archived proposals beyond correcting clearly misleading current references.
- No code behavior changes except where documentation exposes a confirmed contract mismatch.
