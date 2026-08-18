# OCR Correction Feedback Loop

## Why

OCR runs (`src/handlers/document_ocr.rs`) and presents results. There
is no "Was this correct?" feedback, no improvement over time, no
training data.

## What Changes

- After the user reviews an OCR result and saves the transaction,
  record `(document_id, ocr_output, user_corrected)` in
  `ocr_corrections`.
- Future runs may use the corpus to fine-tune the prompt or to seed
  rule-based extraction.
- Expose the corpus as a downloadable JSON for self-hosters who want to
  fine-tune externally.

## Capabilities

### New Capabilities

- `ocr-feedback`: OCR correction capture + corpus export.

## Impact

**New files:**
- `migrations/0039_add_ocr_corrections.sql`.
- `src/handlers/document_ocr_feedback.rs`.
- `tests/integration/ocr_feedback.rs`.
