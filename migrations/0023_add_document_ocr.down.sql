-- Reversible: yes
-- DOWN for 0023_add_document_ocr.sql

DROP TABLE IF EXISTS ocr_jobs;
ALTER TABLE documents DROP COLUMN IF EXISTS ocr_status;
ALTER TABLE documents DROP COLUMN IF EXISTS ocr_text;

