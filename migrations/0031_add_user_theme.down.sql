-- Reversible: yes
-- DOWN for 0031_add_user_theme.sql

ALTER TABLE users DROP COLUMN IF EXISTS theme;

