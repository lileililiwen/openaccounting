# ## Context

Generic upload exists; no wizard.

## Goals / Non-Goals

**Goals:**
- Self-serve for non-tech users.

**Non-Goals:**
- OFX/QFX (separate).

## Decisions

- State in session; commit on the last step.
- Saved mappings as JSON in `import_mappings` table.
