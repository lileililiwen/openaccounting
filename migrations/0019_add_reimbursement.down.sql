-- Reversible: yes
-- DOWN for 0019_add_reimbursement.sql

DROP TABLE IF EXISTS reimbursement_claim_lines;
DROP TABLE IF EXISTS reimbursement_claims;
DROP TABLE IF EXISTS reimbursement_receipts;
DROP TYPE IF EXISTS reimbursement_status;
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS chk_account_subtype;
ALTER TABLE accounts ADD CONSTRAINT chk_account_subtype CHECK (
    (type = 'ASSET'     AND subtype IN ('CURRENT','FIXED','INTANGIBLE','OTHER')) OR
    (type = 'LIABILITY' AND subtype IN ('CURRENT','LONG_TERM')) OR
    (type = 'EQUITY'    AND subtype IN ('EQUITY')) OR
    (type = 'INCOME'    AND subtype IN ('OPERATING','NON_OPERATING')) OR
    (type = 'EXPENSE'   AND subtype IN ('OPERATING','NON_OPERATING','COGS'))
);

