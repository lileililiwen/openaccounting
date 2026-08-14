-- Migration 0023: Add document OCR results table
-- Each uploaded document may have at most one OCR result row.
-- The background task writes this row after processing.

CREATE TABLE document_ocr_results (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    extracted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    amount          NUMERIC(18, 4),
    txn_date        DATE,
    merchant        TEXT,
    raw_text        TEXT NOT NULL DEFAULT '',
    engine          TEXT NOT NULL DEFAULT 'tesseract',
    confidence      REAL NOT NULL DEFAULT 0,
    error_message   TEXT,

    CONSTRAINT uq_document_ocr UNIQUE (document_id)
);

CREATE INDEX idx_document_ocr_document_id ON document_ocr_results (document_id);
