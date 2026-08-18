-- Migration 0035: Add csv_import_mappings table (`u4-csv-import-wizard`).
--
-- A "mapping" is a saved column-to-field correspondence for a
-- (filename glob, format) pair. Once saved, the next upload
-- that matches the glob skips the column-mapping step and
-- goes straight to preview.
--
-- `filename_glob` is a SQL LIKE pattern (e.g. 'bank1_%.csv').
-- One row per (ledger_id, name); UNIQUE so re-saving replaces.

CREATE TABLE IF NOT EXISTS csv_import_mappings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    filename_glob   TEXT NOT NULL,
    format          TEXT NOT NULL CHECK (format IN ('csv', 'ofx', 'qif', 'mt940')),
    date_column     INTEGER NOT NULL,
    description_column INTEGER NOT NULL,
    debit_column    INTEGER,
    credit_column   INTEGER,
    account_column  INTEGER,
    payee_column    INTEGER,
    reference_column INTEGER,
    amount_in_column TEXT
        CHECK (amount_in_column IS NULL OR amount_in_column IN ('debit', 'credit')),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, name)
);

CREATE INDEX IF NOT EXISTS idx_csv_import_mappings_ledger
    ON csv_import_mappings (ledger_id);