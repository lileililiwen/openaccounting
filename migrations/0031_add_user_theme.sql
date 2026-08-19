-- Migration 0031: Add user theme preference (`u8-dark-mode`).
--
-- Three states are stored:
--   'system' — follow the OS `prefers-color-scheme` media query
--   'light'  — always light
--   'dark'   — always dark
--
-- The default is `'system'` so existing users keep the old
-- behaviour (no toggle applied, CSS falls back to OS preference).

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS theme TEXT NOT NULL DEFAULT 'system'
    CHECK (theme IN ('system', 'light', 'dark'));
