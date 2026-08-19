-- Reversible: yes
-- DOWN for 0003_add_user_role.sql

ALTER TABLE users DROP COLUMN IF EXISTS is_admin;
ALTER TABLE users DROP COLUMN IF EXISTS role;
DROP TABLE IF EXISTS user_invitations;

