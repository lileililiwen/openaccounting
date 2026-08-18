-- Migration 0033: Add user locale preference (`u7-localization`).
--
-- One of: 'en', 'zh-CN', 'es', 'fr', 'de', 'ja'. Default 'en'
-- so existing users keep their current language until they
-- opt in via /account/locale.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS locale TEXT NOT NULL DEFAULT 'en'
    CHECK (locale IN ('en', 'zh-CN', 'es', 'fr', 'de', 'ja'));