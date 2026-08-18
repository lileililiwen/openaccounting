# ## Context

No probes.

## Goals / Non-Goals

**Goals:**
- Two standard probes.

**Non-Goals:**
- Per-dependency health (out of scope; only PG + disk).

## Decisions

- `/healthz` is process-only.
- `/readyz` checks `SELECT 1` and a temp-file write to DOCUMENTS_DIR.
