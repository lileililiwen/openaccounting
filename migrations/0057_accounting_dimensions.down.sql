-- 0057_accounting_dimensions.down.sql — reverse 0057 (reversible migration).

DROP TABLE IF EXISTS fx_override_audit;
DROP TABLE IF EXISTS recurring_journal_runs;
DROP TABLE IF EXISTS recurring_journal_lines;
DROP TABLE IF EXISTS recurring_journal_templates;
ALTER TABLE postings DROP COLUMN IF EXISTS project_id;
ALTER TABLE postings DROP COLUMN IF EXISTS cost_center_id;
DROP TABLE IF EXISTS projects;
DROP TABLE IF EXISTS cost_centers;
ALTER TABLE ledgers DROP COLUMN IF EXISTS inventory_method;
