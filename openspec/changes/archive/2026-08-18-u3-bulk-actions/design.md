# ## Context

One-row actions only.

## Goals / Non-Goals

**Goals:**
- Standard bulk semantics.

**Non-Goals:**
- Bulk editing of amount / account (too destructive; use edit-void flow).

## Decisions

- Reuse the reversing-entry infrastructure for bulk delete.
