# ## Context

`kind` already exists and is set to 'standard' on create. We extend it.

## Goals / Non-Goals

**Goals:**
- Non-destructive composition.

**Non-Goals:**
- Multi-user concurrent edits of one draft (last write wins).

## Decisions

- Drafts go through `PostingService::create_draft`, which omits the
  period-close check and the audit write.
- Promotion goes through `PostingService::post_draft(id)`, which runs the
  full check and emits the audit row.
