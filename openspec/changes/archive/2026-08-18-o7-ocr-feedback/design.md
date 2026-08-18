# ## Context

OCR is a black box today.

## Goals / Non-Goals

**Goals:**
- Capture corrections.
- Export corpus.

**Non-Goals:**
- Auto-fine-tuning (out of scope; the export is enough).

## Decisions

- One row per apply. JSON columns for both sides.
- Admin-only corpus export.
