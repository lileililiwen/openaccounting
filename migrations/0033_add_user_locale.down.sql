-- Reversible: yes
-- DOWN for 0033_add_user_locale.sql

ALTER TABLE users DROP COLUMN IF EXISTS locale;

