-- Migration 0030: Add ocr_corrections table for the OCR feedback loop
-- (`o7-ocr-feedback`).
--
-- Every successful OCR + apply cycle writes one row. The row holds
-- BOTH the engine's original output AND the final user-edited
-- values so self-hosters can fine-tune external OCR engines or
-- rule-based extractors.
--
-- The row never contains raw image bytes or document contents —
-- only structured fields (amount / date / merchant / account /
-- claim line ids). See `o7-ocr-feedback` spec requirement
-- "Privacy".

CREATE TABLE IF NOT EXISTS ocr_corrections (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id              UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    ledger_id                UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    user_id                  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claim_id                 UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    reimbursement_line_id    UUID NOT NULL REFERENCES reimbursement_lines(id) ON DELETE CASCADE,
    captured_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- OCR engine output (nullable because the engine may have
    -- failed to populate one of the fields).
    ocr_amount               NUMERIC(18, 4),
    ocr_txn_date             DATE,
    ocr_merchant             TEXT,
    ocr_confidence           REAL,

    -- Final user-edited values that were actually persisted to the
    -- reimbursement line.
    final_amount             NUMERIC(18, 4) NOT NULL,
    final_txn_date           DATE NOT NULL,
    final_merchant           TEXT NOT NULL,
    final_account_id         UUID NOT NULL REFERENCES accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_ocr_corrections_captured_at
    ON ocr_corrections (captured_at);
CREATE INDEX IF NOT EXISTS idx_ocr_corrections_ledger
    ON ocr_corrections (ledger_id);
CREATE INDEX IF NOT EXISTS idx_ocr_corrections_document
    ON ocr_corrections (document_id);
