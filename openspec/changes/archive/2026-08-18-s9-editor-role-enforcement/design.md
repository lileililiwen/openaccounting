# ## Context

The sharing capability allows three roles. The current handlers
unevenly enforce them. This change brings every write path to a single
consistent rule: owner or editor can write; only the owner can change
sharing.

## Goals / Non-Goals

**Goals:**
- Predictable behavior. Editors can co-author a ledger.

**Non-Goals:**
- Custom roles beyond the three already defined.

## Decisions

- A single audit-and-fix pass.
- For each handler we add a tiny integration test verifying the role
  matrix.

## Risks / Trade-offs

- Behavior changes: editors can now create, edit, delete. Existing
  deployments may have relied on the stricter (incorrect) behavior.
  Mitigation: document in the migration guide; default-new-ledgers still
  have a single owner.
