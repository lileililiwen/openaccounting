# ocr-feedback Specification

## Purpose
TBD - created by archiving change o7-ocr-feedback. Update Purpose after archive.
## Requirements
### Requirement: Capture

MUST record every successful OCR + apply cycle with the original OCR output and the final user-edited values.

#### Scenario: Save captures

- **WHEN** the user applies an OCR result and saves the transaction
- **THEN** one row is written to `ocr_corrections`.

### Requirement: Privacy

MUST NOT include the document bytes; only structured fields (vendor, amount, date, account suggestion).

#### Scenario: No bytes

- **WHEN** an admin queries `ocr_corrections`
- **THEN** no binary data is present.

### Requirement: Corpus Export

MUST expose the corpus as `/admin/ocr-corpus.json` for self-hosters who want to fine-tune.

#### Scenario: Export

- **WHEN** an admin downloads the corpus
- **THEN** the file is a JSON array of records.

### Requirement: Opt-Out

MUST allow disabling capture via `OCR_FEEDBACK=false`.

#### Scenario: Disabled

- **WHEN** OCR_FEEDBACK=false
- **THEN** no rows are written.

### Requirement: Analytics

MUST publish a metric `ocr_corrections_total` and a per-field disagreement rate so admins see where the engine is weak.

#### Scenario: Metric

- **WHEN** the user corrects the amount 50% of the time
- **THEN** the metric `ocr_amount_disagreement_rate` reflects it.

