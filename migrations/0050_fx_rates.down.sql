DROP TABLE IF EXISTS fx_revaluations;
ALTER TABLE postings DROP COLUMN IF EXISTS foreign_currency;
ALTER TABLE postings DROP COLUMN IF EXISTS foreign_amount;
DROP INDEX IF EXISTS idx_fx_rates_lookup;
DROP TABLE IF EXISTS fx_rates;
