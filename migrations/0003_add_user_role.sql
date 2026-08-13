-- ============================================================================
-- 0003_add_user_role.sql — Add admin role to users
-- ============================================================================
-- Adds a `role` column with CHECK constraint: 'user' (default) or 'admin'.
-- Promotes the first registered user to admin.
-- ============================================================================

-- Add the role column with a safe default.
ALTER TABLE users
  ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
  CHECK (role IN ('user', 'admin'));

-- Promote the first registered user to admin.
-- If no users exist, this is a no-op.
UPDATE users
  SET role = 'admin'
WHERE id = (SELECT id FROM users ORDER BY created_at LIMIT 1);
